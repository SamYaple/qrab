//! Guest workload identity: pre-boot vTPM provisioning and the read-only
//! `qrab-identity` disk (docs/spire-vm.md, "Guest workload identity").
//!
//! Before the first boot of a VM with [`GuestIdentity::enabled`] the manager
//! runs a private swtpm on the VM's retained TPM state and, over its command
//! socket:
//!
//! 1. derives the EK from TCG template L-1 (what `tpm2_createek -G rsa` and
//!    SPIRE's `tpm_devid` agent regenerate) and has qrab-tpm-issuer certify
//!    it; the DER certificate goes to NV index `0x01c00002`, defined and
//!    write-locked with platform authorization;
//! 2. creates an RSA-2048 RSASSA-SHA256 LDevID under SPIRE's
//!    `SRKTemplateHighRSA` (template H-1: owner hierarchy, empty unique) and
//!    has the issuer sign its DevID certificate (CN `qrab-vm-<uid>`).
//!
//! The blobs and certificates are kept under `identity/guest/` on the state
//! volume, with a [`MARKER`] written last; a start that finds it makes no TPM
//! or issuer call. A migration target never provisions: it installs the
//! source's [`Material`] from the migration offer, and its TPM state arrives
//! in QEMU's stream.
//!
//! The EK is a function of the TPM's endorsement seed, which nothing here
//! changes. Retained TPM state without the marker (a VM that ran before it
//! asked for guest identity) therefore keeps its EK: provisioning adds the
//! LDevID and replaces only a platform-created EK certificate at
//! `0x01c00002` (the old one is kept as `previous-ek-cert.der`). An
//! owner-defined index there, or a hierarchy password the guest set, refuses
//! with an error instead.
//!
//! Every start rebuilds the disk ([`write_image`]): the trust bundle may have
//! rotated, while keys and certificates come from the state volume.
use super::{seed::FAT_EPOCH, Config, GuestIdentity};
use crate::{Error, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, UnixStream},
    process::{Child, Command},
    time::timeout,
};

/// Where the guest mounts the disk; `agent.conf` names files under it.
pub const MOUNT: &str = "/run/qrab-identity";
/// The virtio-blk serial (`/sys/block/vd*/serial`) and QEMU device id.
pub const SERIAL: &str = "qrab-identity";
/// FAT volume label, for `mount LABEL=QRAB-ID`.
pub const LABEL: &str = "QRAB-ID";
/// The TCG EK Credential Profile index of the RSA-2048 EK certificate.
pub const EK_CERT_INDEX: u32 = 0x01c0_0002;
/// Written last by a complete provisioning.
pub const MARKER: &str = "provisioned.json";
/// The disk image, rebuilt in the guest directory on every start.
pub const IMAGE: &str = "identity.img";
/// What the guest's SPIRE agent keeps across reboots.
pub const AGENT_DATA_DIR: &str = "/var/lib/spire/agent";

/// The retained guest-identity directory of a VM's state.
pub fn guest_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("identity").join("guest")
}

/// The disk image QEMU attaches.
pub fn image_path(state_dir: &Path) -> PathBuf {
    guest_dir(state_dir).join(IMAGE)
}

/// Read-only virtio-blk with serial `qrab-identity` and no boot index.
pub(super) fn arguments(state_dir: &Path, args: &mut Vec<String>) {
    let path = image_path(state_dir);
    args.extend([
        "-blockdev".into(),
        serde_json::json!({"driver":"file", "filename":path, "node-name":"identity-file", "read-only":true}).to_string(),
        "-blockdev".into(),
        serde_json::json!({"driver":"raw", "file":"identity-file", "node-name":"identity", "read-only":true}).to_string(),
        "-device".into(),
        format!("virtio-blk-pci,drive=identity,id={SERIAL},serial={SERIAL}"),
    ]);
}

fn failure(message: impl std::fmt::Display) -> Error {
    Error::Config(format!("guest identity: {message}"))
}

fn event(value: serde_json::Value) {
    eprintln!("{value}");
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn pem(label: &str, der: &[u8]) -> String {
    let body = STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for line in body.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(line).expect("base64 is ASCII"));
        out.push('\n');
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

/// The DER of the first PEM block.
fn pem_der(text: &str) -> Option<Vec<u8>> {
    let start = text.find("-----BEGIN ")?;
    let body = &text[start..];
    let body = &body[body.find('\n')? + 1..];
    let end = body.find("-----END ")?;
    let joined: String = body[..end].split_whitespace().collect();
    STANDARD.decode(joined).ok()
}

/// TPM 2.0 commands over swtpm's raw command socket (TPM 2.0 Library Part 3),
/// with empty-password (`TPM_RS_PW`) authorization only.
mod tpm2 {
    use super::failure;
    use crate::Result;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::UnixStream,
    };

    pub const RH_OWNER: u32 = 0x4000_0001;
    const RS_PW: u32 = 0x4000_0009;
    pub const RH_ENDORSEMENT: u32 = 0x4000_000B;
    pub const RH_PLATFORM: u32 = 0x4000_000C;
    const ST_NO_SESSIONS: u16 = 0x8001;
    const ST_SESSIONS: u16 = 0x8002;
    /// `TPM_RC_HANDLE` for the first handle: an undefined NV index.
    const RC_HANDLE_1: u32 = 0x18B;
    /// Below libtpms' `MAX_NV_BUFFER_SIZE` (1024).
    const NV_CHUNK: usize = 512;
    const RESPONSE_LIMIT: usize = 8192;

    const ALG_RSA: u16 = 0x0001;
    const ALG_AES: u16 = 0x0006;
    const ALG_SHA256: u16 = 0x000B;
    const ALG_NULL: u16 = 0x0010;
    const ALG_RSASSA: u16 = 0x0014;
    const ALG_CFB: u16 = 0x0043;

    // TPMA_OBJECT (Part 2, §8.3).
    const FIXED_TPM: u32 = 1 << 1;
    const FIXED_PARENT: u32 = 1 << 4;
    const SENSITIVE_DATA_ORIGIN: u32 = 1 << 5;
    const USER_WITH_AUTH: u32 = 1 << 6;
    const ADMIN_WITH_POLICY: u32 = 1 << 7;
    const NO_DA: u32 = 1 << 10;
    const RESTRICTED: u32 = 1 << 16;
    const DECRYPT: u32 = 1 << 17;
    const SIGN: u32 = 1 << 18;

    // TPMA_NV (Part 2, §13.4).
    pub const NV_PPWRITE: u32 = 1 << 0;
    pub const NV_POLICY_DELETE: u32 = 1 << 10;
    pub const NV_WRITELOCKED: u32 = 1 << 11;
    pub const NV_WRITEDEFINE: u32 = 1 << 13;
    pub const NV_PPREAD: u32 = 1 << 16;
    pub const NV_OWNERREAD: u32 = 1 << 17;
    pub const NV_AUTHREAD: u32 = 1 << 18;
    pub const NV_NO_DA: u32 = 1 << 25;
    pub const NV_WRITTEN: u32 = 1 << 29;
    pub const NV_PLATFORMCREATE: u32 = 1 << 30;
    /// What swtpm_setup gives an EK certificate index, so the guest's TSS
    /// and SPIRE (`NV_Read` with the index's own empty auth) can read it.
    pub const EK_CERT_NV: u32 = NV_PLATFORMCREATE
        | NV_AUTHREAD
        | NV_OWNERREAD
        | NV_PPREAD
        | NV_PPWRITE
        | NV_NO_DA
        | NV_WRITEDEFINE;

    mod cc {
        pub const NV_UNDEFINE_SPACE: u32 = 0x122;
        pub const NV_DEFINE_SPACE: u32 = 0x12A;
        pub const CREATE_PRIMARY: u32 = 0x131;
        pub const NV_WRITE: u32 = 0x137;
        pub const NV_WRITE_LOCK: u32 = 0x138;
        pub const SHUTDOWN: u32 = 0x145;
        pub const NV_READ: u32 = 0x14E;
        pub const CREATE: u32 = 0x153;
        pub const LOAD: u32 = 0x157;
        pub const FLUSH_CONTEXT: u32 = 0x165;
        pub const NV_READ_PUBLIC: u32 = 0x169;
    }

    fn name(cc: u32) -> &'static str {
        match cc {
            cc::NV_UNDEFINE_SPACE => "NV_UndefineSpace",
            cc::NV_DEFINE_SPACE => "NV_DefineSpace",
            cc::CREATE_PRIMARY => "CreatePrimary",
            cc::NV_WRITE => "NV_Write",
            cc::NV_WRITE_LOCK => "NV_WriteLock",
            cc::SHUTDOWN => "Shutdown",
            cc::NV_READ => "NV_Read",
            cc::CREATE => "Create",
            cc::LOAD => "Load",
            cc::FLUSH_CONTEXT => "FlushContext",
            cc::NV_READ_PUBLIC => "NV_ReadPublic",
            _ => "command",
        }
    }

    /// The default EK policy, `PolicySecret(TPM_RH_ENDORSEMENT)`.
    const EK_AUTH_POLICY: [u8; 32] = [
        0x83, 0x71, 0x97, 0x67, 0x44, 0x84, 0xb3, 0xf8, 0x1a, 0x90, 0xcc, 0x8d, 0x46, 0xa5, 0xd7,
        0x24, 0xfd, 0x52, 0xd7, 0x6e, 0x06, 0x52, 0x0b, 0x64, 0xf2, 0xa1, 0xda, 0x1b, 0x33, 0x14,
        0x69, 0xaa,
    ];

    fn tpm2b(out: &mut Vec<u8>, bytes: &[u8]) {
        out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
        out.extend_from_slice(bytes);
    }

    /// An RSA-2048 `TPMT_PUBLIC`, SHA-256 name, exponent 65537.
    fn rsa(
        attributes: u32,
        policy: &[u8],
        symmetric: bool,
        scheme: Option<(u16, u16)>,
        unique: usize,
    ) -> Vec<u8> {
        let mut t = Vec::new();
        t.extend_from_slice(&ALG_RSA.to_be_bytes());
        t.extend_from_slice(&ALG_SHA256.to_be_bytes());
        t.extend_from_slice(&attributes.to_be_bytes());
        tpm2b(&mut t, policy);
        if symmetric {
            for v in [ALG_AES, 128, ALG_CFB] {
                t.extend_from_slice(&v.to_be_bytes());
            }
        } else {
            t.extend_from_slice(&ALG_NULL.to_be_bytes());
        }
        match scheme {
            Some((alg, hash)) => {
                t.extend_from_slice(&alg.to_be_bytes());
                t.extend_from_slice(&hash.to_be_bytes());
            }
            None => t.extend_from_slice(&ALG_NULL.to_be_bytes()),
        }
        t.extend_from_slice(&2048u16.to_be_bytes());
        t.extend_from_slice(&0u32.to_be_bytes());
        tpm2b(&mut t, &vec![0; unique]);
        t
    }

    /// TCG EK Credential Profile template L-1 (go-tpm-tools
    /// `DefaultEKTemplateRSA`): the EK SPIRE regenerates.
    pub fn ek_template() -> Vec<u8> {
        rsa(
            FIXED_TPM
                | FIXED_PARENT
                | SENSITIVE_DATA_ORIGIN
                | ADMIN_WITH_POLICY
                | RESTRICTED
                | DECRYPT,
            &EK_AUTH_POLICY,
            true,
            None,
            256,
        )
    }

    /// SPIRE v1.15.3 `tpmutil.SRKTemplateHighRSA` (template H-1): go-tpm-tools
    /// `SRKTemplateRSA` with an empty unique field.
    pub fn srk_template() -> Vec<u8> {
        rsa(
            FIXED_TPM
                | FIXED_PARENT
                | SENSITIVE_DATA_ORIGIN
                | USER_WITH_AUTH
                | NO_DA
                | RESTRICTED
                | DECRYPT,
            &[],
            true,
            None,
            0,
        )
    }

    /// The LDevID: an unrestricted RSASSA-SHA256 signing key bound to this
    /// TPM and SRK. Its empty password is no DA protection, so it is noDA:
    /// guest mistakes cannot lock SPIRE out.
    pub fn ldevid_template() -> Vec<u8> {
        rsa(
            FIXED_TPM | FIXED_PARENT | SENSITIVE_DATA_ORIGIN | USER_WITH_AUTH | NO_DA | SIGN,
            &[],
            false,
            Some((ALG_RSASSA, ALG_SHA256)),
            0,
        )
    }

    pub struct NvPublic {
        pub attributes: u32,
        pub size: u16,
    }

    struct Reader<'a>(&'a [u8]);

    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> Result<&'a [u8]> {
            if self.0.len() < n {
                return Err(failure("truncated TPM response"));
            }
            let (head, rest) = self.0.split_at(n);
            self.0 = rest;
            Ok(head)
        }
        fn u16(&mut self) -> Result<u16> {
            let b = self.take(2)?;
            Ok(u16::from_be_bytes([b[0], b[1]]))
        }
        fn u32(&mut self) -> Result<u32> {
            let b = self.take(4)?;
            Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        }
        fn tpm2b(&mut self) -> Result<&'a [u8]> {
            let n = self.u16()?;
            self.take(n.into())
        }
    }

    struct Response {
        handle: Option<u32>,
        params: Vec<u8>,
    }

    fn hint(rc: u32) -> &'static str {
        // Format-one AUTH_FAIL / BAD_AUTH: a hierarchy password is set.
        if rc & 0x80 != 0 && matches!(rc & 0x3F, 0x0E | 0x22) {
            " (a hierarchy or object password is set, likely by the guest; pre-boot provisioning needs the TPM's empty default)"
        } else {
            ""
        }
    }

    pub struct Tpm(UnixStream);

    impl Tpm {
        pub fn new(stream: UnixStream) -> Self {
            Self(stream)
        }

        async fn raw(
            &mut self,
            cc: u32,
            handles: &[u32],
            authorized: bool,
            out_handle: bool,
            params: &[u8],
        ) -> Result<(u32, Response)> {
            let mut body = Vec::new();
            body.extend_from_slice(&cc.to_be_bytes());
            for handle in handles {
                body.extend_from_slice(&handle.to_be_bytes());
            }
            if authorized {
                // One TPMS_AUTH_COMMAND: TPM_RS_PW, empty nonce, no
                // attributes, empty password.
                body.extend_from_slice(&9u32.to_be_bytes());
                body.extend_from_slice(&RS_PW.to_be_bytes());
                body.extend_from_slice(&[0, 0, 0, 0, 0]);
            }
            body.extend_from_slice(params);
            let tag = if authorized {
                ST_SESSIONS
            } else {
                ST_NO_SESSIONS
            };
            let mut command = Vec::with_capacity(body.len() + 6);
            command.extend_from_slice(&tag.to_be_bytes());
            command.extend_from_slice(&((body.len() + 6) as u32).to_be_bytes());
            command.extend_from_slice(&body);
            self.0.write_all(&command).await?;
            let mut header = [0u8; 10];
            self.0.read_exact(&mut header).await?;
            let tag = u16::from_be_bytes([header[0], header[1]]);
            let size = u32::from_be_bytes([header[2], header[3], header[4], header[5]]) as usize;
            let rc = u32::from_be_bytes([header[6], header[7], header[8], header[9]]);
            if !(10..=RESPONSE_LIMIT).contains(&size) {
                return Err(failure(format!("TPM2_{} response size {size}", name(cc))));
            }
            let mut rest = vec![0u8; size - 10];
            self.0.read_exact(&mut rest).await?;
            if rc != 0 {
                return Ok((
                    rc,
                    Response {
                        handle: None,
                        params: Vec::new(),
                    },
                ));
            }
            let mut r = Reader(&rest);
            let handle = if out_handle { Some(r.u32()?) } else { None };
            let params = if tag == ST_SESSIONS {
                let n = r.u32()? as usize;
                r.take(n)?.to_vec()
            } else {
                r.0.to_vec()
            };
            Ok((rc, Response { handle, params }))
        }

        async fn call(
            &mut self,
            cc: u32,
            handles: &[u32],
            authorized: bool,
            out_handle: bool,
            params: &[u8],
        ) -> Result<Response> {
            match self
                .raw(cc, handles, authorized, out_handle, params)
                .await?
            {
                (0, response) => Ok(response),
                (rc, _) => Err(failure(format!(
                    "TPM2_{} failed with TPM_RC 0x{rc:03x}{}",
                    name(cc),
                    hint(rc)
                ))),
            }
        }

        fn create_params(template: &[u8]) -> Vec<u8> {
            // inSensitive (empty userAuth and data), inPublic, outsideInfo,
            // creationPCR (no selection).
            let mut p = vec![0, 4, 0, 0, 0, 0];
            tpm2b(&mut p, template);
            p.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
            p
        }

        /// A primary key's handle and its `TPMT_PUBLIC`.
        pub async fn create_primary(
            &mut self,
            hierarchy: u32,
            template: &[u8],
        ) -> Result<(u32, Vec<u8>)> {
            let r = self
                .call(
                    cc::CREATE_PRIMARY,
                    &[hierarchy],
                    true,
                    true,
                    &Self::create_params(template),
                )
                .await?;
            let public = Reader(&r.params).tpm2b()?.to_vec();
            Ok((r.handle.expect("object handle"), public))
        }

        /// A child key's `TPM2B_PRIVATE` body and `TPMT_PUBLIC`: both without
        /// their size prefixes, as SPIRE's `devid_priv_path` and
        /// `devid_pub_path` want them.
        pub async fn create(&mut self, parent: u32, template: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
            let r = self
                .call(
                    cc::CREATE,
                    &[parent],
                    true,
                    false,
                    &Self::create_params(template),
                )
                .await?;
            let mut reader = Reader(&r.params);
            let private = reader.tpm2b()?.to_vec();
            let public = reader.tpm2b()?.to_vec();
            Ok((private, public))
        }

        pub async fn load(&mut self, parent: u32, private: &[u8], public: &[u8]) -> Result<u32> {
            let mut p = Vec::new();
            tpm2b(&mut p, private);
            tpm2b(&mut p, public);
            let r = self.call(cc::LOAD, &[parent], true, true, &p).await?;
            Ok(r.handle.expect("object handle"))
        }

        pub async fn flush(&mut self, handle: u32) -> Result<()> {
            self.call(cc::FLUSH_CONTEXT, &[], false, false, &handle.to_be_bytes())
                .await?;
            Ok(())
        }

        pub async fn nv_read_public(&mut self, index: u32) -> Result<Option<NvPublic>> {
            match self
                .raw(cc::NV_READ_PUBLIC, &[index], false, false, &[])
                .await?
            {
                (0, r) => {
                    let mut outer = Reader(&r.params);
                    let mut public = Reader(outer.tpm2b()?);
                    let _index = public.u32()?;
                    let _name_alg = public.u16()?;
                    let attributes = public.u32()?;
                    let _policy = public.tpm2b()?;
                    let size = public.u16()?;
                    Ok(Some(NvPublic { attributes, size }))
                }
                (RC_HANDLE_1, _) => Ok(None),
                (rc, _) => Err(failure(format!(
                    "TPM2_NV_ReadPublic failed with TPM_RC 0x{rc:03x}"
                ))),
            }
        }

        /// Read a whole index, authorized by `auth` (platform, owner or the
        /// index itself).
        pub async fn nv_read(&mut self, auth: u32, index: u32, size: u16) -> Result<Vec<u8>> {
            let mut out = Vec::with_capacity(size.into());
            while out.len() < usize::from(size) {
                let n = (usize::from(size) - out.len()).min(NV_CHUNK) as u16;
                let mut p = Vec::new();
                p.extend_from_slice(&n.to_be_bytes());
                p.extend_from_slice(&(out.len() as u16).to_be_bytes());
                let r = self
                    .call(cc::NV_READ, &[auth, index], true, false, &p)
                    .await?;
                out.extend_from_slice(Reader(&r.params).tpm2b()?);
            }
            Ok(out)
        }

        pub async fn nv_define(&mut self, index: u32, attributes: u32, size: u16) -> Result<()> {
            let mut public = Vec::new();
            public.extend_from_slice(&index.to_be_bytes());
            public.extend_from_slice(&ALG_SHA256.to_be_bytes());
            public.extend_from_slice(&attributes.to_be_bytes());
            tpm2b(&mut public, &[]);
            public.extend_from_slice(&size.to_be_bytes());
            let mut p = Vec::new();
            tpm2b(&mut p, &[]);
            tpm2b(&mut p, &public);
            self.call(cc::NV_DEFINE_SPACE, &[RH_PLATFORM], true, false, &p)
                .await?;
            Ok(())
        }

        pub async fn nv_write(&mut self, index: u32, data: &[u8]) -> Result<()> {
            for (n, chunk) in data.chunks(NV_CHUNK).enumerate() {
                let mut p = Vec::new();
                tpm2b(&mut p, chunk);
                p.extend_from_slice(&((n * NV_CHUNK) as u16).to_be_bytes());
                self.call(cc::NV_WRITE, &[RH_PLATFORM, index], true, false, &p)
                    .await?;
            }
            Ok(())
        }

        pub async fn nv_write_lock(&mut self, index: u32) -> Result<()> {
            self.call(cc::NV_WRITE_LOCK, &[RH_PLATFORM, index], true, false, &[])
                .await?;
            Ok(())
        }

        pub async fn nv_undefine(&mut self, index: u32) -> Result<()> {
            self.call(
                cc::NV_UNDEFINE_SPACE,
                &[RH_PLATFORM, index],
                true,
                false,
                &[],
            )
            .await?;
            Ok(())
        }

        /// `TPM2_Shutdown(TPM_SU_CLEAR)`: an orderly stop before swtpm exits.
        pub async fn shutdown(&mut self) -> Result<()> {
            self.call(cc::SHUTDOWN, &[], false, false, &[0, 0]).await?;
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn templates_follow_the_tcg_and_spire_profiles() {
            let ek = ek_template();
            // type, nameAlg, attributes 0x000300b2 (the swtpm/tpm2-tools EK).
            assert_eq!(&ek[..8], &[0, 1, 0, 0x0b, 0, 3, 0, 0xb2]);
            assert_eq!(ek.len(), 4 + 4 + 34 + 6 + 2 + 2 + 4 + 2 + 256);
            let srk = srk_template();
            assert_eq!(&srk[4..8], &0x0003_0472u32.to_be_bytes());
            // Empty authPolicy, AES-128-CFB, NULL scheme, 2048, exponent 0, empty unique.
            assert_eq!(
                &srk[8..],
                &[0, 0, 0, 6, 0, 0x80, 0, 0x43, 0, 0x10, 8, 0, 0, 0, 0, 0, 0, 0]
            );
            let ldevid = ldevid_template();
            assert_eq!(&ldevid[4..8], &0x0004_0472u32.to_be_bytes());
            assert_eq!(
                &ldevid[8..],
                &[0, 0, 0, 0x10, 0, 0x14, 0, 0x0b, 8, 0, 0, 0, 0, 0, 0, 0]
            );
        }
    }
}

/// Mutual-TLS HTTP/1.1 client for qrab-tpm-issuer's two endpoints.
struct Issuer {
    connector: qrab_spiffe::Connector,
    host: String,
    port: u16,
}

/// The issuer's success body (`qrab_tpm_issuer::IssueResponse`).
#[derive(Deserialize)]
struct IssueResponse {
    certificate: String,
    certificate_pem: String,
    ca_certificate_pem: String,
    sha256: String,
    existing: bool,
}

struct Certificate {
    der: Vec<u8>,
    pem: String,
    ca_pem: String,
    existing: bool,
}

const HTTP_LIMIT: usize = 64 << 10;
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const ATTEMPTS: u32 = 5;

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// `(status, body)` of one response; the body is bounded by Content-Length,
/// chunked framing or the end of the stream.
async fn read_response<R: AsyncRead + Unpin>(stream: &mut R) -> std::io::Result<(u16, Vec<u8>)> {
    let invalid = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_string());
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut eof = false;
    let fill = |buf: &mut Vec<u8>, chunk: &[u8]| -> std::io::Result<()> {
        if buf.len() + chunk.len() > HTTP_LIMIT {
            return Err(invalid("issuer response too large"));
        }
        buf.extend_from_slice(chunk);
        Ok(())
    };
    let head = loop {
        if let Some(end) = find(&buf, b"\r\n\r\n") {
            break end;
        }
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err(invalid("issuer closed the connection before a response"));
        }
        fill(&mut buf, &chunk[..n])?;
    };
    let text = std::str::from_utf8(&buf[..head]).map_err(|_| invalid("non-UTF-8 headers"))?;
    let mut lines = text.split("\r\n");
    let status: u16 = lines
        .next()
        .and_then(|l| {
            l.strip_prefix("HTTP/1.1 ")
                .or_else(|| l.strip_prefix("HTTP/1.0 "))
        })
        .and_then(|l| l.get(..3))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| invalid("malformed status line"))?;
    let mut length = None;
    let mut chunked = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| invalid("bad Content-Length"))?,
            );
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            chunked = value.eq_ignore_ascii_case("chunked");
        }
    }
    let start = head + 4;
    let complete = |buf: &[u8]| match length {
        Some(n) => buf.len() >= start + n,
        None if chunked => find(&buf[start..], b"0\r\n\r\n").is_some(),
        None => false,
    };
    while !complete(&buf) && !eof {
        match stream.read(&mut chunk).await {
            Ok(0) => eof = true,
            Ok(n) => fill(&mut buf, &chunk[..n])?,
            // A peer that closes without close_notify after a full body.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => eof = true,
            Err(e) => return Err(e),
        }
    }
    let body = &buf[start..];
    let body = match length {
        Some(n) if body.len() >= n => body[..n].to_vec(),
        Some(_) => return Err(invalid("issuer response truncated")),
        None if chunked => {
            let mut out = Vec::new();
            let mut rest = body;
            loop {
                let line = find(rest, b"\r\n").ok_or_else(|| invalid("bad chunk"))?;
                let size = std::str::from_utf8(&rest[..line])
                    .ok()
                    .and_then(|s| usize::from_str_radix(s.split(';').next()?.trim(), 16).ok())
                    .ok_or_else(|| invalid("bad chunk size"))?;
                rest = &rest[line + 2..];
                if size == 0 {
                    break out;
                }
                if rest.len() < size + 2 {
                    return Err(invalid("chunk truncated"));
                }
                out.extend_from_slice(&rest[..size]);
                rest = &rest[size + 2..];
            }
        }
        None => body.to_vec(),
    };
    Ok((status, body))
}

impl Issuer {
    fn new(settings: &GuestIdentity, identity: &qrab_spiffe::Identity) -> Result<Self> {
        let (host, port) = settings.issuer_address()?;
        let peer = qrab_spiffe::id(&settings.issuer_id).map_err(failure)?;
        let connector = identity.connector(&[peer]).map_err(failure)?;
        Ok(Self {
            connector,
            host,
            port,
        })
    }

    async fn once(&self, path: &str, body: &[u8]) -> std::io::Result<(u16, Vec<u8>)> {
        let tcp = timeout(
            Duration::from_secs(10),
            TcpStream::connect((self.host.as_str(), self.port)),
        )
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "connect timed out"))??;
        let mut tls = self
            .connector
            .connect(tcp)
            .await
            .map_err(std::io::Error::other)?;
        let host = if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        };
        let head = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        tls.write_all(head.as_bytes()).await?;
        tls.write_all(body).await?;
        tls.flush().await?;
        read_response(&mut tls).await
    }

    /// POST with retries on transport errors and 5xx; 4xx is final.
    async fn post(&self, path: &str, body: serde_json::Value) -> Result<Certificate> {
        let body = serde_json::to_vec(&body)?;
        let mut delay = Duration::from_secs(1);
        let mut last = String::new();
        for attempt in 1..=ATTEMPTS {
            match timeout(HTTP_TIMEOUT, self.once(path, &body)).await {
                Ok(Ok((200, response))) => return Self::certificate(&response),
                Ok(Ok((status, response))) => {
                    let reason = serde_json::from_slice::<serde_json::Value>(&response)
                        .ok()
                        .and_then(|v| v.get("error")?.as_str().map(str::to_owned))
                        .unwrap_or_else(|| String::from_utf8_lossy(&response).into_owned());
                    last = format!("qrab-tpm-issuer {path} answered {status}: {reason}");
                    if status < 500 {
                        return Err(failure(last));
                    }
                }
                Ok(Err(error)) => last = format!("qrab-tpm-issuer {path}: {error}"),
                Err(_) => last = format!("qrab-tpm-issuer {path} timed out"),
            }
            event(
                serde_json::json!({"event":"guest_identity_issuer_retry","attempt":attempt,"error":last}),
            );
            if attempt < ATTEMPTS {
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
        }
        Err(failure(last))
    }

    fn certificate(body: &[u8]) -> Result<Certificate> {
        let response: IssueResponse = serde_json::from_slice(body)?;
        let der = STANDARD
            .decode(&response.certificate)
            .map_err(|e| failure(format!("issuer certificate is not base64: {e}")))?;
        if sha256_hex(&der) != response.sha256
            || pem_der(&response.certificate_pem).as_deref() != Some(&der[..])
            || pem_der(&response.ca_certificate_pem).is_none()
        {
            return Err(failure("issuer response certificate fields disagree"));
        }
        Ok(Certificate {
            der,
            pem: response.certificate_pem,
            ca_pem: response.ca_certificate_pem,
            existing: response.existing,
        })
    }
}

/// The certified key's modulus must be inside the certificate's public key.
fn certifies(certificate: &[u8], tpmt_public: &[u8]) -> bool {
    use x509_cert::der::Decode;
    let Ok(cert) = x509_cert::Certificate::from_der(certificate) else {
        return false;
    };
    let spki = cert
        .tbs_certificate
        .subject_public_key_info
        .subject_public_key
        .raw_bytes();
    // TPMT_PUBLIC ends with the 256-byte modulus (TPM2B_PUBLIC_KEY_RSA).
    tpmt_public.len() > 256 && {
        let modulus = &tpmt_public[tpmt_public.len() - 256..];
        spki.windows(256).any(|w| w == modulus)
    }
}

/// The record written last by a complete provisioning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub version: u32,
    /// SHA-256 of the EK's `TPMT_PUBLIC`.
    pub ek_public_sha256: String,
    pub ek_certificate_sha256: String,
    pub devid_certificate_sha256: String,
    /// A platform-created certificate at 0x01c00002 was replaced; the old
    /// one is `previous-ek-cert.der`.
    pub replaced_ek_certificate: bool,
}

impl Marker {
    pub fn read(guest: &Path) -> Result<Option<Self>> {
        match fs::read(guest.join(MARKER)) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// What [`provision`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provisioned {
    /// The marker was present: no TPM or issuer call was made.
    Retained(Marker),
    /// The TPM was provisioned now.
    New(Marker),
}

/// The VM's retained TPM and guest-identity directories, and where the
/// provisioning swtpm's socket may go.
pub struct Vtpm<'a> {
    pub swtpm_binary: &'a Path,
    pub tpm_state: &'a Path,
    pub guest: &'a Path,
    pub socket_dir: &'a Path,
}

fn private_dir(path: &Path) -> Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            if !fs::symlink_metadata(path)?.is_dir() {
                return Err(failure(format!("{} is not a directory", path.display())));
            }
        }
        Err(e) => return Err(e.into()),
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

/// Replace `dir/name` atomically and durably (mode 0600).
fn store(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let mut file = tempfile::Builder::new()
        .prefix(&format!(".{name}."))
        .permissions(fs::Permissions::from_mode(0o600))
        .tempfile_in(dir)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(dir.join(name))
        .map_err(|e| Error::Io(e.error))?;
    fs::File::open(dir)?.sync_all()?;
    Ok(())
}

fn stored(dir: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    match fs::read(dir.join(name)) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// A private swtpm on the retained state, with a raw command socket.
struct Session {
    child: Child,
    tpm: tpm2::Tpm,
}

impl Session {
    async fn start(vtpm: &Vtpm<'_>) -> Result<Self> {
        let socket = vtpm.socket_dir.join("swtpm-identity.sock");
        if socket.as_os_str().len() > 107 {
            return Err(failure("socket directory is too long for a Unix socket"));
        }
        let _ = fs::remove_file(&socket);
        let mut child = Command::new(vtpm.swtpm_binary)
            .args(["socket", "--tpm2", "--tpmstate"])
            .arg(format!("dir={},mode=0600", vtpm.tpm_state.display()))
            .arg("--server")
            .arg(format!("type=unixio,path={},mode=0600", socket.display()))
            // swtpm itself sends TPM2_Startup(CLEAR): no QEMU, no CMD_INIT.
            .args(["--flags", "not-need-init,startup-clear"])
            .env_remove("TPM_PORT")
            .env_remove("TPM_PATH")
            .env_remove("NOTIFY_SOCKET")
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| failure(format!("cannot run {}: {e}", vtpm.swtpm_binary.display())))?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        let stream = loop {
            if let Some(exit) = child.try_wait()? {
                return Err(failure(format!("provisioning swtpm exited: {exit}")));
            }
            match UnixStream::connect(&socket).await {
                Ok(stream) => break stream,
                Err(_) if tokio::time::Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(25)).await
                }
                Err(e) => return Err(failure(format!("provisioning swtpm socket: {e}"))),
            }
        };
        Ok(Self {
            child,
            tpm: tpm2::Tpm::new(stream),
        })
    }

    /// Orderly TPM shutdown, then wait for swtpm to release the state.
    /// libtpms has stored its NV state before it answers `TPM2_Shutdown`,
    /// so a SIGKILL after an orderly shutdown loses nothing.
    async fn stop(mut self) -> Result<()> {
        let orderly = self.tpm.shutdown().await;
        if super::tpm::terminate(&mut self.child, Duration::from_secs(15))
            .await
            .is_none()
        {
            self.child.start_kill()?;
            self.child.wait().await?;
            event(
                serde_json::json!({"event":"guest_identity_swtpm_killed","orderly":orderly.is_ok()}),
            );
        }
        drop(self.tpm);
        orderly
    }
}

/// Put `der` at 0x01c00002. True when an earlier, different certificate
/// was replaced.
async fn write_ek_certificate(tpm: &mut tpm2::Tpm, guest: &Path, der: &[u8]) -> Result<bool> {
    use tpm2::*;
    let size =
        u16::try_from(der.len()).map_err(|_| failure("EK certificate is larger than 64 KiB"))?;
    let mut replaced = false;
    if let Some(nv) = tpm.nv_read_public(EK_CERT_INDEX).await? {
        let auth = if nv.attributes & NV_PPREAD != 0 {
            Some(RH_PLATFORM)
        } else if nv.attributes & NV_AUTHREAD != 0 {
            Some(EK_CERT_INDEX)
        } else if nv.attributes & NV_OWNERREAD != 0 {
            Some(RH_OWNER)
        } else {
            None
        };
        let current = match auth {
            Some(auth) if nv.attributes & NV_WRITTEN != 0 => {
                Some(tpm.nv_read(auth, EK_CERT_INDEX, nv.size).await?)
            }
            _ => None,
        };
        if current.as_deref() == Some(der) {
            if nv.attributes & NV_WRITELOCKED == 0 && nv.attributes & NV_WRITEDEFINE != 0 {
                tpm.nv_write_lock(EK_CERT_INDEX).await?;
            }
            return Ok(false);
        }
        if nv.attributes & NV_PLATFORMCREATE == 0 || nv.attributes & NV_POLICY_DELETE != 0 {
            return Err(failure(
                "NV index 0x01c00002 is owner-defined or policy-delete; the platform cannot replace its EK certificate, so this TPM cannot get guest identity",
            ));
        }
        if let Some(previous) = current {
            if stored(guest, "previous-ek-cert.der")?.is_none() {
                store(guest, "previous-ek-cert.der", &previous)?;
            }
        }
        tpm.nv_undefine(EK_CERT_INDEX).await?;
        replaced = true;
    }
    tpm.nv_define(EK_CERT_INDEX, EK_CERT_NV, size).await?;
    tpm.nv_write(EK_CERT_INDEX, der).await?;
    tpm.nv_write_lock(EK_CERT_INDEX).await?;
    Ok(replaced)
}

/// Provision the vTPM unless [`MARKER`] says it already is. Crash-safe: the
/// LDevID blobs are stored before the issuer certifies them and reused by a
/// retry, and the issuer returns its stored certificate for the same key.
pub async fn provision(
    vtpm: &Vtpm<'_>,
    settings: &GuestIdentity,
    identity: &qrab_spiffe::Identity,
) -> Result<Provisioned> {
    private_dir(vtpm.guest)?;
    if let Some(marker) = Marker::read(vtpm.guest)? {
        return Ok(Provisioned::Retained(marker));
    }
    let issuer = Issuer::new(settings, identity)?;
    let mut session = Session::start(vtpm).await?;
    let result = provision_with(&mut session.tpm, vtpm.guest, &issuer).await;
    let stopped = session.stop().await;
    let marker = result?;
    stopped?;
    store(vtpm.guest, MARKER, &serde_json::to_vec_pretty(&marker)?)?;
    event(
        serde_json::json!({"event":"guest_identity_provisioned","ek_public_sha256":marker.ek_public_sha256,"devid_certificate_sha256":marker.devid_certificate_sha256,"replaced_ek_certificate":marker.replaced_ek_certificate}),
    );
    Ok(Provisioned::New(marker))
}

async fn provision_with(tpm: &mut tpm2::Tpm, guest: &Path, issuer: &Issuer) -> Result<Marker> {
    let (ek, ek_public) = tpm
        .create_primary(tpm2::RH_ENDORSEMENT, &tpm2::ek_template())
        .await?;
    tpm.flush(ek).await?;
    if let Some(previous) = stored(guest, "ek.pub")? {
        if previous != ek_public {
            return Err(failure(
                "the TPM's EK differs from the one recorded in identity/guest/ek.pub; refusing to certify a different EK",
            ));
        }
    }
    store(guest, "ek.pub", &ek_public)?;
    let ek_b64 = STANDARD.encode(&ek_public);
    let ek_cert = issuer
        .post("/v1/ek", serde_json::json!({ "ek_public": ek_b64 }))
        .await?;
    if !certifies(&ek_cert.der, &ek_public) {
        return Err(failure(
            "the issued EK certificate does not certify this EK",
        ));
    }
    store(guest, "ek.der", &ek_cert.der)?;
    store(guest, "ek.pem", ek_cert.pem.as_bytes())?;
    store(guest, "ek-ca.pem", ek_cert.ca_pem.as_bytes())?;
    let replaced = write_ek_certificate(tpm, guest, &ek_cert.der).await?;

    let (srk, _) = tpm
        .create_primary(tpm2::RH_OWNER, &tpm2::srk_template())
        .await?;
    let keys = match (stored(guest, "devid.pub")?, stored(guest, "devid.priv")?) {
        // A retry reuses the key an interrupted run already stored, if it
        // still loads under this SRK.
        (Some(public), Some(private)) => match tpm.load(srk, &private, &public).await {
            Ok(handle) => {
                tpm.flush(handle).await?;
                Some((private, public))
            }
            Err(_) => None,
        },
        _ => None,
    };
    let public = match keys {
        Some((_, public)) => public,
        None => {
            let (private, public) = tpm.create(srk, &tpm2::ldevid_template()).await?;
            store(guest, "devid.priv", &private)?;
            store(guest, "devid.pub", &public)?;
            public
        }
    };
    tpm.flush(srk).await?;
    let devid = issuer
        .post(
            "/v1/devid",
            serde_json::json!({ "ldevid_public": STANDARD.encode(&public), "ek_public": ek_b64 }),
        )
        .await?;
    if !certifies(&devid.der, &public) {
        return Err(failure(
            "the issued DevID certificate does not certify this LDevID",
        ));
    }
    store(guest, "devid.der", &devid.der)?;
    store(guest, "devid.pem", devid.pem.as_bytes())?;
    store(guest, "devid-ca.pem", devid.ca_pem.as_bytes())?;
    event(
        serde_json::json!({"event":"guest_identity_certified","ek_existing":ek_cert.existing,"devid_existing":devid.existing}),
    );
    Ok(Marker {
        version: 1,
        ek_public_sha256: sha256_hex(&ek_public),
        ek_certificate_sha256: sha256_hex(&ek_cert.der),
        devid_certificate_sha256: sha256_hex(&devid.der),
        replaced_ek_certificate: replaced,
    })
}

/// The trust domain's X.509 bundle, for the guest agent's
/// `trust_bundle_path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    pub trust_domain: String,
    pub pem: String,
}

impl Bundle {
    pub fn of(identity: &qrab_spiffe::Identity) -> Result<Self> {
        let id = identity.id().map_err(failure)?;
        let set = identity
            .source()
            .bundle_set()
            .map_err(|e| failure(format!("trust bundle: {e}")))?;
        let bundle = set
            .get(id.trust_domain())
            .ok_or_else(|| failure("the Workload API has no bundle for its own trust domain"))?;
        let pem: String = bundle
            .authorities()
            .iter()
            .map(|c| pem("CERTIFICATE", c.as_ref()))
            .collect();
        if pem.is_empty() {
            return Err(failure("the trust bundle is empty"));
        }
        Ok(Self {
            trust_domain: id.trust_domain_name().to_owned(),
            pem,
        })
    }

    fn store(&self, guest: &Path) -> Result<()> {
        store(guest, "bundle.pem", self.pem.as_bytes())?;
        store(guest, "trust-domain", self.trust_domain.as_bytes())
    }

    fn stored(guest: &Path) -> Result<Option<Self>> {
        match (stored(guest, "bundle.pem")?, stored(guest, "trust-domain")?) {
            (Some(pem), Some(td)) => Ok(Some(Self {
                pem: String::from_utf8(pem).map_err(|_| failure("bundle.pem is not UTF-8"))?,
                trust_domain: String::from_utf8(td)
                    .map_err(|_| failure("trust-domain is not UTF-8"))?,
            })),
            _ => Ok(None),
        }
    }
}

/// The guest SPIRE agent configuration.
///
/// ```
/// let conf = manager::pod::identity::agent_conf("yapplesauce.com");
/// assert!(conf.contains(r#"trust_bundle_path = "/run/qrab-identity/bundle.pem""#));
/// assert!(conf.contains(r#"server_address = "127.0.0.1""#));
/// assert!(conf.contains(r#"NodeAttestor "tpm_devid""#));
/// assert!(conf.contains(r#"devid_pub_path = "/run/qrab-identity/devid.pub""#));
/// ```
pub fn agent_conf(trust_domain: &str) -> String {
    format!(
        r#"# Generated by qrab-manager for the qrab-identity disk; rebuilt every VM start.
agent {{
    data_dir = "{AGENT_DATA_DIR}"
    log_level = "INFO"
    # qrab-spire-relay forwards this over the org.yapplesauce.spire.0 port.
    server_address = "127.0.0.1"
    server_port = "8081"
    socket_path = "/run/spire-agent/public/api.sock"
    trust_bundle_path = "{MOUNT}/bundle.pem"
    trust_domain = "{trust_domain}"
}}

plugins {{
    NodeAttestor "tpm_devid" {{
        plugin_data {{
            tpm_device_path = "/dev/tpmrm0"
            devid_cert_path = "{MOUNT}/devid.pem"
            devid_priv_path = "{MOUNT}/devid.priv"
            devid_pub_path = "{MOUNT}/devid.pub"
        }}
    }}

    KeyManager "disk" {{
        plugin_data {{
            directory = "{AGENT_DATA_DIR}"
        }}
    }}

    WorkloadAttestor "unix" {{
        plugin_data {{}}
    }}
}}
"#
    )
}

const START: &str = r#"#!/bin/sh
# Start qrab-spire-relay and spire-agent from the qrab-identity disk, which
# must already be mounted at /run/qrab-identity (see README).
set -eu
here=/run/qrab-identity
tool() { command -v "$1" 2>/dev/null || echo "$here/tools/$1"; }
relay=$(tool qrab-spire-relay)
agent=$(tool spire-agent)
# The relay listens on 127.0.0.1:8081 only.
ip link set lo up 2>/dev/null || ifconfig lo 127.0.0.1 up 2>/dev/null || true
[ -e /dev/tpmrm0 ] || modprobe tpm_tis 2>/dev/null || modprobe tpm_crb 2>/dev/null || true
mkdir -p /var/lib/spire/agent /run/spire-agent/public
(while :; do "$relay" </dev/null; sleep 1; done) &
sleep 1
exec "$agent" run -config "$here/agent.conf"
"#;

const README: &str = r#"qrab-identity: this VM's SPIRE agent identity (read-only, rebuilt on every start)

Mount it, then start the agent:

    mkdir -p /run/qrab-identity
    for d in /sys/block/vd*; do
        [ "$(cat "$d/serial" 2>/dev/null)" = qrab-identity ] && dev=/dev/${d##*/}
    done
    mount -t vfat -o ro "${dev:-LABEL=QRAB-ID}" /run/qrab-identity
    sh /run/qrab-identity/start.sh &

start.sh runs qrab-spire-relay (127.0.0.1:8081, over the virtio-serial port
org.yapplesauce.spire.0) and `spire-agent run -config agent.conf`, preferring
binaries on PATH over tools/. The agent needs /dev/tpmrm0 (tpm_tis) and keeps
its state in /var/lib/spire/agent; keep that directory across reboots so the
agent keeps its SVID instead of re-attesting.

Files:
  agent.conf   SPIRE agent configuration (tpm_devid node attestation)
  bundle.pem   the trust domain's X.509 bundle (trust_bundle_path)
  devid.pem    the DevID certificate, CN qrab-vm-<uid> (devid_cert_path)
  devid.pub    the LDevID TPMT_PUBLIC (devid_pub_path)
  devid.priv   the LDevID TPM2B_PRIVATE body, sealed to this vTPM (devid_priv_path)
  ek.pem       the EK certificate, also at TPM NV index 0x01c00002
  tools/       static spire-agent and qrab-spire-relay, when the host has them
"#;

/// The files at the disk's root, in write order.
fn root_files(guest: &Path, bundle: &Bundle) -> Result<Vec<(&'static str, Vec<u8>)>> {
    let read = |name: &str| {
        stored(guest, name)?.ok_or_else(|| failure(format!("identity/guest/{name} is missing")))
    };
    Ok(vec![
        ("README", README.as_bytes().to_vec()),
        ("start.sh", START.as_bytes().to_vec()),
        ("agent.conf", agent_conf(&bundle.trust_domain).into_bytes()),
        ("bundle.pem", bundle.pem.clone().into_bytes()),
        ("devid.pem", read("devid.pem")?),
        ("devid.pub", read("devid.pub")?),
        ("devid.priv", read("devid.priv")?),
        ("ek.pem", read("ek.pem")?),
    ])
}

/// Regular files directly in `dir`, by name; none when it does not exist.
fn tools(dir: &Path) -> Result<Vec<(String, PathBuf, u64)>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        let metadata = fs::metadata(entry.path())?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if metadata.is_file() && !name.starts_with('.') {
            out.push((name, entry.path(), metadata.len()));
        }
    }
    out.sort();
    Ok(out)
}

/// Build the FAT disk at `out` from the stored keys and certificates, the
/// bundle and `tools_dir`. Deterministic for the same inputs (FAT-epoch
/// timestamps, fixed order), so a migration target rebuilds the source's
/// bytes; the image replaces `out` atomically.
pub fn write_image(guest: &Path, bundle: &Bundle, tools_dir: &Path, out: &Path) -> Result<()> {
    let files = root_files(guest, bundle)?;
    let tools = tools(tools_dir)?;
    let content: u64 = files.iter().map(|(_, b)| b.len() as u64).sum::<u64>()
        + tools.iter().map(|(_, _, n)| n).sum::<u64>();
    let count = (files.len() + tools.len()) as u64;
    const MIB: u64 = 1 << 20;
    // Room for the FATs, directories and each file's last partial cluster.
    let size = (content + content / 16 + count * 64 * 1024 + 2 * MIB)
        .div_ceil(MIB)
        .max(4)
        * MIB;
    let dir = out
        .parent()
        .ok_or_else(|| failure("image path has no parent"))?;
    let mut file = tempfile::Builder::new()
        .prefix(".identity-img.")
        .permissions(fs::Permissions::from_mode(0o600))
        .tempfile_in(dir)?;
    file.as_file().set_len(size)?;
    let mut label = [b' '; 11];
    label[..LABEL.len()].copy_from_slice(LABEL.as_bytes());
    fatfs::format_volume(
        file.as_file_mut(),
        fatfs::FormatVolumeOptions::new().volume_label(label),
    )?;
    {
        let volume = fatfs::FileSystem::new(
            file.as_file_mut(),
            fatfs::FsOptions::new().time_provider(&FAT_EPOCH),
        )?;
        {
            let root = volume.root_dir();
            for (name, bytes) in &files {
                root.create_file(name)?.write_all(bytes)?;
            }
            if !tools.is_empty() {
                let dir = root.create_dir("tools")?;
                for (name, path, _) in &tools {
                    let mut source = fs::File::open(path)?;
                    std::io::copy(&mut source, &mut dir.create_file(name)?)?;
                }
            }
        }
        volume.unmount()?;
    }
    file.as_file().sync_all()?;
    file.persist(out).map_err(|e| Error::Io(e.error))?;
    Ok(())
}

/// Files a migration target needs to rebuild the source's disk and to keep
/// the source's provisioning record.
const MIGRATED: [&str; 11] = [
    "ek.pub",
    "ek.der",
    "ek.pem",
    "ek-ca.pem",
    "devid.pub",
    "devid.priv",
    "devid.pem",
    "devid-ca.pem",
    "bundle.pem",
    "trust-domain",
    MARKER,
];

/// The source's guest-identity files (base64), carried in the migration
/// offer. `devid.priv` is wrapped by the vTPM's SRK and loads only in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Material {
    pub files: BTreeMap<String, String>,
}

/// The source side: `None` unless this VM has the identity disk.
pub(super) fn material(config: &Config) -> Result<Option<Material>> {
    if !config.guest_identity.enabled {
        return Ok(None);
    }
    let guest = guest_dir(&config.state_dir);
    let mut files = BTreeMap::new();
    for name in MIGRATED {
        let bytes = stored(&guest, name)?
            .ok_or_else(|| failure(format!("identity/guest/{name} is missing")))?;
        files.insert(name.to_owned(), STANDARD.encode(bytes));
    }
    Ok(Some(Material { files }))
}

/// The target side: write the source's files, refusing anything else.
pub fn install(guest: &Path, material: &Material) -> Result<()> {
    private_dir(guest)?;
    if material.files.len() != MIGRATED.len()
        || !MIGRATED
            .iter()
            .all(|name| material.files.contains_key(*name))
    {
        return Err(failure(
            "the migration offer's identity files are incomplete",
        ));
    }
    // The marker last, as in a provisioning.
    for name in MIGRATED {
        let bytes = STANDARD
            .decode(&material.files[name])
            .map_err(|e| failure(format!("offered {name}: {e}")))?;
        store(guest, name, &bytes)?;
    }
    Marker::read(guest)?.ok_or_else(|| failure("the offered marker is missing"))?;
    Ok(())
}

async fn workload_identity(settings: &GuestIdentity) -> Result<qrab_spiffe::Identity> {
    match &settings.workload_api {
        Some(endpoint) => qrab_spiffe::Identity::connect(endpoint).await,
        None => qrab_spiffe::Identity::from_env().await,
    }
    .map_err(failure)
}

/// Before swtpm and QEMU start: provision (or, on a migration target,
/// install the source's `offered` material) and rebuild the disk.
pub(super) async fn prepare(config: &Config, offered: Option<&Material>) -> Result<()> {
    let settings = &config.guest_identity;
    let guest = guest_dir(&config.state_dir);
    let tpm = config
        .tpm
        .as_ref()
        .ok_or_else(|| failure("guest identity needs a TPM"))?;
    let bundle = match offered {
        Some(material) => {
            install(&guest, material)?;
            Bundle::stored(&guest)?.ok_or_else(|| failure("the offered bundle is missing"))?
        }
        None => {
            let identity = workload_identity(settings).await;
            if Marker::read(&guest)?.is_none() {
                let identity = identity
                    .as_ref()
                    .map_err(|e| Error::Config(e.to_string()))?;
                let vtpm = Vtpm {
                    swtpm_binary: &tpm.swtpm_binary,
                    tpm_state: &config.state_dir.join("identity").join("tpm"),
                    guest: &guest,
                    socket_dir: &config.runtime_dir,
                };
                if let Err(error) = provision(&vtpm, settings, identity).await {
                    event(
                        serde_json::json!({"event":"guest_identity_refused","reason":error.to_string()}),
                    );
                    return Err(error);
                }
            }
            match identity.and_then(|identity| Bundle::of(&identity)) {
                Ok(bundle) => {
                    bundle.store(&guest)?;
                    bundle
                }
                Err(error) => {
                    let bundle = Bundle::stored(&guest)?
                        .ok_or_else(|| failure(format!("no trust bundle: {error}")))?;
                    event(
                        serde_json::json!({"event":"guest_identity_stale_bundle","error":error.to_string()}),
                    );
                    bundle
                }
            }
        }
    };
    write_image(
        &guest,
        &bundle,
        &settings.guest_tools_dir,
        &image_path(&config.state_dir),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn guest(dir: &Path) -> PathBuf {
        let guest = dir.join("guest");
        private_dir(&guest).unwrap();
        for (name, bytes) in [
            (
                "devid.pem",
                &b"-----BEGIN CERTIFICATE-----\nAA==\n-----END CERTIFICATE-----\n"[..],
            ),
            ("devid.pub", &[0, 1, 2][..]),
            ("devid.priv", &[3, 4][..]),
            ("ek.pem", &b"ek"[..]),
        ] {
            store(&guest, name, bytes).unwrap();
        }
        guest
    }

    fn read_back(path: &Path) -> BTreeMap<String, Vec<u8>> {
        let fs =
            fatfs::FileSystem::new(fs::File::open(path).unwrap(), fatfs::FsOptions::new()).unwrap();
        assert_eq!(fs.volume_label(), LABEL);
        let mut out = BTreeMap::new();
        let mut walk = vec![(String::new(), fs.root_dir())];
        while let Some((prefix, dir)) = walk.pop() {
            for entry in dir.iter() {
                let entry = entry.unwrap();
                let name = format!("{prefix}{}", entry.file_name());
                if entry.is_dir() {
                    if !matches!(entry.file_name().as_str(), "." | "..") {
                        walk.push((format!("{name}/"), entry.to_dir()));
                    }
                } else {
                    let mut bytes = Vec::new();
                    entry.to_file().read_to_end(&mut bytes).unwrap();
                    out.insert(name, bytes);
                }
            }
        }
        out
    }

    fn bundle() -> Bundle {
        Bundle {
            trust_domain: "yapplesauce.com".into(),
            pem: "-----BEGIN CERTIFICATE-----\nAQ==\n-----END CERTIFICATE-----\n".into(),
        }
    }

    #[test]
    fn the_disk_holds_the_agent_files_and_optional_tools() {
        let dir = tempfile::tempdir().unwrap();
        let guest = guest(dir.path());
        let out = guest.join(IMAGE);
        write_image(&guest, &bundle(), &dir.path().join("absent"), &out).unwrap();
        let files = read_back(&out);
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "README",
                "agent.conf",
                "bundle.pem",
                "devid.pem",
                "devid.priv",
                "devid.pub",
                "ek.pem",
                "start.sh"
            ]
        );
        assert_eq!(files["devid.pub"], [0, 1, 2]);
        assert_eq!(files["devid.priv"], [3, 4]);
        assert_eq!(
            files["agent.conf"],
            agent_conf("yapplesauce.com").into_bytes()
        );
        assert_eq!(fs::metadata(&out).unwrap().len(), 4 << 20);
        // Deterministic: the same inputs give the same bytes.
        let again = guest.join("again.img");
        write_image(&guest, &bundle(), &dir.path().join("absent"), &again).unwrap();
        assert_eq!(fs::read(&out).unwrap(), fs::read(&again).unwrap());

        // Tools (~6 MiB here) grow the image and land under tools/.
        let tools = dir.path().join("tools");
        fs::create_dir(&tools).unwrap();
        fs::write(tools.join("spire-agent"), vec![7u8; 6 << 20]).unwrap();
        fs::write(tools.join("qrab-spire-relay"), b"relay").unwrap();
        fs::write(tools.join(".hidden"), b"x").unwrap();
        write_image(&guest, &bundle(), &tools, &out).unwrap();
        let files = read_back(&out);
        assert_eq!(files["tools/spire-agent"].len(), 6 << 20);
        assert_eq!(files["tools/qrab-spire-relay"], b"relay");
        assert!(!files.contains_key("tools/.hidden"));
        assert!(fs::metadata(&out).unwrap().len() > 6 << 20);
    }

    #[test]
    fn a_missing_key_refuses_the_disk() {
        let dir = tempfile::tempdir().unwrap();
        let guest = guest(dir.path());
        fs::remove_file(guest.join("devid.priv")).unwrap();
        let error = write_image(&guest, &bundle(), dir.path(), &guest.join(IMAGE)).unwrap_err();
        assert!(error.to_string().contains("devid.priv is missing"));
    }

    #[test]
    fn material_round_trips_and_refuses_extra_or_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut config: Config = serde_json::from_value(serde_json::json!({
            "kernel":"/unused","state_dir":dir.path().join("state"),"guest_identity":{"enabled":true}}))
        .unwrap();
        let source = guest_dir(&config.state_dir);
        fs::create_dir_all(&source).unwrap();
        for name in MIGRATED {
            store(&source, name, name.as_bytes()).unwrap();
        }
        let marker = Marker {
            version: 1,
            ek_public_sha256: "a".into(),
            ek_certificate_sha256: "b".into(),
            devid_certificate_sha256: "c".into(),
            replaced_ek_certificate: false,
        };
        store(&source, MARKER, &serde_json::to_vec(&marker).unwrap()).unwrap();
        let material = material(&config).unwrap().unwrap();
        let target = dir.path().join("target");
        install(&target, &material).unwrap();
        assert_eq!(Marker::read(&target).unwrap(), Some(marker));
        assert_eq!(fs::read(target.join("devid.priv")).unwrap(), b"devid.priv");

        let mut extra = material.clone();
        extra.files.insert("../escape".into(), String::new());
        assert!(install(&dir.path().join("t2"), &extra).is_err());
        let mut missing = material.clone();
        missing.files.remove("devid.priv");
        assert!(install(&dir.path().join("t3"), &missing).is_err());

        config.guest_identity.enabled = false;
        assert_eq!(super::material(&config).unwrap(), None);
    }

    #[tokio::test]
    async fn responses_are_framed_by_length_or_chunks() {
        let fixed = b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\nhello trailing";
        assert_eq!(
            read_response(&mut &fixed[..]).await.unwrap(),
            (200, b"hello".to_vec())
        );
        let chunked = b"HTTP/1.1 409 Conflict\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n";
        assert_eq!(
            read_response(&mut &chunked[..]).await.unwrap(),
            (409, b"abcde".to_vec())
        );
        let short = b"HTTP/1.1 200 OK\r\ncontent-length: 9\r\n\r\nabc";
        assert!(read_response(&mut &short[..]).await.is_err());
        assert!(read_response(&mut &b"garbage\r\n\r\n"[..]).await.is_err());
    }

    #[test]
    fn pem_round_trips() {
        let der = (0..200u8).collect::<Vec<_>>();
        let text = pem("CERTIFICATE", &der);
        assert!(text.lines().all(|l| l.len() <= 64));
        assert_eq!(pem_der(&text).unwrap(), der);
    }

    #[test]
    fn arguments_attach_a_read_only_serial_disk() {
        let mut args = Vec::new();
        arguments(Path::new("/var/lib/qrab"), &mut args);
        assert_eq!(args[0], "-blockdev");
        assert!(args[1].contains("/var/lib/qrab/identity/guest/identity.img"));
        assert!(args[1].contains("\"read-only\":true") && args[3].contains("\"read-only\":true"));
        assert_eq!(
            args[5],
            "virtio-blk-pci,drive=identity,id=qrab-identity,serial=qrab-identity"
        );
    }
}
