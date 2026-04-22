/** @file
  QrabSbAutoEnroll — auto-invoke OvmfPkg/EnrollDefaultKeys.efi when PK
  is absent at DXE dispatch.

  Upstream EnrollDefaultKeys is a UEFI_APPLICATION (shell app). In an
  unattended AmdSev SNP guest there is no shell, so the firmware never
  transitions out of Setup Mode; Secure Boot is compiled in but
  permissive. This DXE driver closes the gap by locating
  EnrollDefaultKeys.efi in the FV via its FILE_GUID and running it
  directly.

  - Idempotent: if `PK` is present, we return immediately.
  - Best-effort: any LoadImage/StartImage error is logged and
    returned as EFI_SUCCESS so BDS continues (the smoke then reports
    SETUP_MODE and the operator can diagnose).

  Copyright (c) 2026 qrab contributors.
  SPDX-License-Identifier: BSD-2-Clause-Patent
**/
#include <Uefi.h>
#include <Guid/EventGroup.h>
#include <Guid/GlobalVariable.h>
#include <Library/BaseMemoryLib.h>
#include <Library/DebugLib.h>
#include <Library/DevicePathLib.h>
#include <Library/MemoryAllocationLib.h>
#include <Library/UefiBootServicesTableLib.h>
#include <Library/UefiLib.h>
#include <Library/UefiRuntimeServicesTableLib.h>
#include <Library/PrintLib.h>
#include <Protocol/LoadedImage.h>
#include <Protocol/FirmwareVolume2.h>
#include <Protocol/ShellParameters.h>

//
// FILE_GUID of OvmfPkg/EnrollDefaultKeys/EnrollDefaultKeys.inf — the
// application we chain-load. Must match the upstream .inf exactly;
// edk2 rejects a mismatch at FV dispatch time.
//
STATIC CONST EFI_GUID mEnrollDefaultKeysFileGuid = {
  0xA0BAA8A3, 0x041D, 0x48A8,
  { 0xBC, 0x87, 0xC3, 0x6D, 0x12, 0x1B, 0x5E, 0x3D }
};

//
// Check whether `PK` is enrolled. Returns TRUE iff the variable is
// present and non-empty.
//
STATIC
BOOLEAN
IsPkEnrolled (
  VOID
  )
{
  EFI_STATUS  Status;
  UINTN       Size;

  Size   = 0;
  Status = gRT->GetVariable (
                  EFI_PLATFORM_KEY_NAME,
                  &gEfiGlobalVariableGuid,
                  NULL,
                  &Size,
                  NULL
                  );
  // EFI_BUFFER_TOO_SMALL means the var exists, size > 0.
  // EFI_NOT_FOUND means the var is absent — Setup Mode.
  if (Status == EFI_BUFFER_TOO_SMALL && Size > 0) {
    return TRUE;
  }
  return FALSE;
}

//
// Build a MEDIA_FW_VOL_FILEPATH_DEVICE_PATH pointing at `FileGuid` in
// `FvHandle`. Caller frees the returned pointer via FreePool.
//
STATIC
EFI_DEVICE_PATH_PROTOCOL *
BuildFvFileDevicePath (
  IN EFI_HANDLE    FvHandle,
  IN CONST EFI_GUID  *FileGuid
  )
{
  EFI_DEVICE_PATH_PROTOCOL       *FvDp;
  MEDIA_FW_VOL_FILEPATH_DEVICE_PATH  FileNode;

  FvDp = DevicePathFromHandle (FvHandle);
  if (FvDp == NULL) {
    return NULL;
  }

  EfiInitializeFwVolDevicepathNode (&FileNode, FileGuid);
  return AppendDevicePathNode (FvDp, (EFI_DEVICE_PATH_PROTOCOL *)&FileNode);
}

//
// Scan installed FV2 protocols for one that carries `FileGuid`.
//
STATIC
EFI_HANDLE
FindFvContainingFile (
  IN CONST EFI_GUID  *FileGuid
  )
{
  EFI_STATUS                     Status;
  EFI_HANDLE                     *Handles;
  UINTN                          HandleCount;
  UINTN                          Idx;
  EFI_FIRMWARE_VOLUME2_PROTOCOL  *Fv;
  UINTN                          BufSize;
  UINT32                         AuthStatus;
  EFI_FV_FILETYPE                FoundType;
  EFI_FV_FILE_ATTRIBUTES         FoundAttr;

  Handles     = NULL;
  HandleCount = 0;
  Status = gBS->LocateHandleBuffer (
                  ByProtocol,
                  &gEfiFirmwareVolume2ProtocolGuid,
                  NULL,
                  &HandleCount,
                  &Handles
                  );
  if (EFI_ERROR (Status)) {
    return NULL;
  }

  for (Idx = 0; Idx < HandleCount; Idx++) {
    Status = gBS->HandleProtocol (
                    Handles[Idx],
                    &gEfiFirmwareVolume2ProtocolGuid,
                    (VOID **)&Fv
                    );
    if (EFI_ERROR (Status)) {
      continue;
    }
    BufSize = 0;
    Status = Fv->ReadFile (
                   Fv,
                   FileGuid,
                   NULL,
                   &BufSize,
                   &FoundType,
                   &FoundAttr,
                   &AuthStatus
                   );
    if (Status == EFI_BUFFER_TOO_SMALL || Status == EFI_SUCCESS) {
      EFI_HANDLE  Result = Handles[Idx];
      FreePool (Handles);
      return Result;
    }
  }
  FreePool (Handles);
  return NULL;
}

//
// Stored at driver entry so the ReadyToBoot callback can LoadImage
// with the correct ParentImageHandle.
//
STATIC EFI_HANDLE  mDriverImageHandle;

STATIC
VOID
EFIAPI
QrabSbAutoEnrollDoIt (
  IN EFI_EVENT  Event,
  IN VOID       *Context
  )
{
  EFI_STATUS                Status;
  EFI_HANDLE                FvHandle;
  EFI_DEVICE_PATH_PROTOCOL  *ImagePath;
  EFI_HANDLE                EnrollImage;
  UINTN                     ExitDataSize;
  CHAR16                    *ExitData;

  gBS->CloseEvent (Event);

  // Fires at ReadyToBoot, so ConOut/ConIn are up and EnrollDefaultKeys'
  // AsciiPrint() calls will not ASSERT. Driver logs via DEBUG_ERROR
  // (AmdSev PcdDebugPrintErrorLevel 0x8000004F) so RELEASE builds see
  // the trace too.
  DEBUG ((DEBUG_ERROR, "QrabSbAutoEnroll: entry (PK %a)\n",
    IsPkEnrolled () ? "already-enrolled" : "absent"));

  if (IsPkEnrolled ()) {
    return;
  }

  FvHandle = FindFvContainingFile (&mEnrollDefaultKeysFileGuid);
  if (FvHandle == NULL) {
    DEBUG ((DEBUG_ERROR, "QrabSbAutoEnroll: EnrollDefaultKeys.efi NOT in any FV\n"));
    return;
  }
  DEBUG ((DEBUG_ERROR, "QrabSbAutoEnroll: found EnrollDefaultKeys.efi in FV %p\n",
    FvHandle));

  ImagePath = BuildFvFileDevicePath (FvHandle, &mEnrollDefaultKeysFileGuid);
  if (ImagePath == NULL) {
    DEBUG ((DEBUG_ERROR, "QrabSbAutoEnroll: device-path build failed\n"));
    return;
  }

  Status = gBS->LoadImage (FALSE, mDriverImageHandle, ImagePath, NULL, 0, &EnrollImage);
  FreePool (ImagePath);
  if (EFI_ERROR (Status)) {
    DEBUG ((DEBUG_ERROR, "QrabSbAutoEnroll: LoadImage failed: %r\n", Status));
    return;
  }

  // EnrollDefaultKeys is a UEFI_APPLICATION that pulls argc/argv from
  // gEfiShellParametersProtocol via ShellCEntryLib. Without that
  // protocol installed on its image handle, ShellCEntryLib hits an
  // ASSERT. Install a minimal one with Argc=1, Argv=["EnrollDefaultKeys"],
  // and NULL std{in,out,err} — the app does not consult those and we
  // specifically want the default (i.e. not --no-default) code path.
  STATIC CHAR16 *mShellArgv[] = { L"EnrollDefaultKeys" };
  STATIC EFI_SHELL_PARAMETERS_PROTOCOL mShellParams = {
    mShellArgv,
    1,
    NULL,
    NULL,
    NULL
  };
  Status = gBS->InstallProtocolInterface (
                  &EnrollImage,
                  &gEfiShellParametersProtocolGuid,
                  EFI_NATIVE_INTERFACE,
                  &mShellParams
                  );
  if (EFI_ERROR (Status)) {
    DEBUG ((DEBUG_ERROR,
      "QrabSbAutoEnroll: InstallProtocolInterface(ShellParameters) failed: %r\n",
      Status));
    gBS->UnloadImage (EnrollImage);
    return;
  }

  ExitDataSize = 0;
  ExitData     = NULL;
  Status       = gBS->StartImage (EnrollImage, &ExitDataSize, &ExitData);
  if (ExitData != NULL) {
    FreePool (ExitData);
  }
  if (EFI_ERROR (Status)) {
    DEBUG ((DEBUG_ERROR, "QrabSbAutoEnroll: StartImage returned %r\n", Status));
    return;
  }

  DEBUG ((DEBUG_ERROR, "QrabSbAutoEnroll: EnrollDefaultKeys completed; PK %a\n",
    IsPkEnrolled () ? "NOW ENROLLED" : "still absent"));
}

EFI_STATUS
EFIAPI
QrabSbAutoEnrollEntry (
  IN EFI_HANDLE        ImageHandle,
  IN EFI_SYSTEM_TABLE  *SystemTable
  )
{
  EFI_STATUS  Status;
  EFI_EVENT   ReadyToBoot;

  mDriverImageHandle = ImageHandle;

  // ReadyToBoot fires after ConOut/ConIn are installed but before the
  // BDS picks its boot option. EnrollDefaultKeys.efi will find working
  // gST->ConOut + SMBIOS and can successfully enrol PK/KEK/db without
  // racing other boot decisions.
  Status = gBS->CreateEventEx (
                  EVT_NOTIFY_SIGNAL,
                  TPL_CALLBACK,
                  QrabSbAutoEnrollDoIt,
                  NULL,
                  &gEfiEventReadyToBootGuid,
                  &ReadyToBoot
                  );
  if (EFI_ERROR (Status)) {
    DEBUG ((DEBUG_ERROR,
      "QrabSbAutoEnroll: CreateEventEx(ReadyToBoot) failed: %r\n",
      Status));
  }
  return EFI_SUCCESS;
}
