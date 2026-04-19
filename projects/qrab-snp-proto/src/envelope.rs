//! AES-256-GCM envelope for `SNP_GUEST_REQUEST`.
//!
//! Encrypt / decrypt a [`SnpGuestMsg`] payload with a VMPCK key,
//! mirroring AMD SEV-SNP ABI Spec §5.4.2:
//!
//!   * AAD covers header offsets `0x30..0x60` (the plaintext
//!     metadata — `algo_id`, `hdr_version`, `hdr_size`, `msg_type`,
//!     `msg_version`, `msg_size`, `msg_vmpck`, reserved fields).
//!   * IV is 12 bytes: `msg_seqno` (u64 LE) followed by four zero
//!     bytes. The counter must increment monotonically per request
//!     (and again per response), reusing an IV across requests with
//!     the same VMPCK is a catastrophic crypto break.
//!   * The tag is 16 bytes, stored in `hdr.auth_tag[0..16]` (the
//!     remaining 16 bytes are reserved).
//!
//! Both `qrab-boot` (UEFI) and host-side unit tests build the same
//! envelope through this module so round-trip coverage is possible
//! on `cargo test`.
//!
//! All routines work in-place to avoid allocating the 4 KiB payload
//! on the heap — the qrab-boot caller already owns a
//! `&mut SnpGuestMsg` backed by DMA-mapped shared memory.

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce, Tag,
    aead::{AeadInPlace, generic_array::GenericArray},
};
use zerocopy::{FromBytes, IntoBytes};

use crate::{
    AEAD_IV_LEN, AEAD_TAG_LEN, MSG_REPORT_REQ_SIZE, SNP_AEAD_AES_256_GCM,
    SNP_GUEST_MSG_AAD_LEN, SNP_GUEST_MSG_AAD_OFFSET, SNP_GUEST_MSG_HDR_VERSION,
    SNP_GUEST_MSG_SIZE, SNP_REPORT_MSG_VERSION, SnpGuestMsg, SnpGuestMsgHdr,
    SnpMsgType, VMPCK_LEN,
};

/// Distinguished error surface for envelope operations. Kept
/// concrete (no `&str`) so both UEFI `log::warn!` sites and host
/// tests can match on variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeError {
    /// Plaintext would not fit in the 4000 B payload region. The
    /// largest legitimate payload is [`crate::MSG_REPORT_RESP_SIZE`]
    /// = 1280 bytes; passing anything larger here is a programmer
    /// error.
    PlaintextTooLarge,
    /// Buffer supplied to [`decrypt_guest_msg`] is smaller than the
    /// declared `msg_size`. Decrypt caller must pass a
    /// `plaintext_out` at least as big as the returned payload.
    OutputBufferTooSmall,
    /// The header's `algo_id` / `hdr_version` / `hdr_size` /
    /// `msg_vmpck` fields did not match the expected values for this
    /// exchange. Caller passed a reply for the wrong key or the PSP
    /// returned a malformed message.
    HeaderMismatch,
    /// `msg_seqno` in the response did not match the value the
    /// caller claimed to expect. Caller must drop the whole reply
    /// on this error to prevent seqno replay.
    SeqnoMismatch,
    /// `msg_type` in the response did not match what the caller
    /// asked for (e.g. got a key-exchange reply for a report
    /// request).
    MsgTypeMismatch,
    /// AES-256-GCM verification failed: either the ciphertext was
    /// altered in transit or the VMPCK is wrong.
    AuthenticationFailed,
}

/// Build a fresh 4 KiB `SnpGuestMsg` envelope containing
/// `plaintext` encrypted under `vmpck` with `msg_seqno` as IV.
///
/// On success:
///
/// * `out.hdr.auth_tag[0..16]` carries the AES-GCM tag; the rest is
///   zeroed.
/// * `out.hdr.{algo_id, hdr_version, hdr_size, msg_type,
///   msg_version, msg_size, msg_vmpck}` are written.
/// * `out.payload` holds the ciphertext for the first
///   `plaintext.len()` bytes, zero padding for the tail.
///
/// Returns `EnvelopeError::PlaintextTooLarge` if the plaintext is
/// bigger than `SNP_GUEST_MSG_SIZE - 96` (the payload region).
pub fn encrypt_guest_msg(
    out: &mut SnpGuestMsg,
    vmpck: &[u8; VMPCK_LEN],
    msg_seqno: u64,
    msg_type: SnpMsgType,
    msg_vmpck: u8,
    plaintext: &[u8],
) -> Result<(), EnvelopeError> {
    if plaintext.len() > SNP_GUEST_MSG_SIZE - 96 {
        return Err(EnvelopeError::PlaintextTooLarge);
    }

    // Zero the whole envelope first so reserved regions stay 0.
    *out = SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE])
        .expect("4 KiB decodes into SnpGuestMsg");

    // Fill in the header's AAD-covered fields + seqno.
    out.hdr.msg_seqno = msg_seqno;
    out.hdr.algo_id = SNP_AEAD_AES_256_GCM;
    out.hdr.hdr_version = SNP_GUEST_MSG_HDR_VERSION;
    out.hdr.hdr_size = 0x60;
    out.hdr.msg_type = msg_type as u8;
    out.hdr.msg_version = SNP_REPORT_MSG_VERSION;
    out.hdr.msg_size = plaintext.len() as u16;
    out.hdr.msg_vmpck = msg_vmpck;

    // Copy plaintext into the payload region; bytes past plaintext
    // remain zero (matches what the PSP sends for short messages).
    out.payload[..plaintext.len()].copy_from_slice(plaintext);

    // Extract the AAD slice from the header and stash a small copy
    // — aes_gcm's `AeadInPlace` needs `&[u8]`, and we cannot borrow
    // from `out` while also mutably borrowing `out.payload`.
    let mut aad = [0u8; SNP_GUEST_MSG_AAD_LEN];
    {
        let hdr_bytes = <SnpGuestMsgHdr as IntoBytes>::as_bytes(&out.hdr);
        aad.copy_from_slice(&hdr_bytes[SNP_GUEST_MSG_AAD_OFFSET..0x60]);
    }

    let iv = SnpGuestMsg::iv_from_seqno(msg_seqno);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(vmpck));
    let nonce = Nonce::from_slice(&iv);

    // Encrypt just the plaintext region; zero-pad tail stays zero.
    let tag = cipher
        .encrypt_in_place_detached(nonce, &aad, &mut out.payload[..plaintext.len()])
        .map_err(|_| EnvelopeError::AuthenticationFailed)?;

    out.hdr.auth_tag[..AEAD_TAG_LEN].copy_from_slice(&tag);
    Ok(())
}

/// Decrypt `msg` back into `plaintext_out`, verifying the AES-GCM
/// tag + the header invariants (algo, hdr_version, msg_type,
/// msg_vmpck, msg_seqno).
///
/// Returns the number of bytes written into `plaintext_out`, which
/// equals `msg.hdr.msg_size` (clamped to whatever fits). The tail of
/// `plaintext_out` past the returned length is left untouched.
pub fn decrypt_guest_msg(
    msg: &SnpGuestMsg,
    vmpck: &[u8; VMPCK_LEN],
    expected_seqno: u64,
    expected_msg_type: SnpMsgType,
    expected_msg_vmpck: u8,
    plaintext_out: &mut [u8],
) -> Result<usize, EnvelopeError> {
    if msg.hdr.algo_id != SNP_AEAD_AES_256_GCM
        || msg.hdr.hdr_version != SNP_GUEST_MSG_HDR_VERSION
        || msg.hdr.hdr_size != 0x60
        || msg.hdr.msg_vmpck != expected_msg_vmpck
    {
        return Err(EnvelopeError::HeaderMismatch);
    }
    if msg.hdr.msg_seqno != expected_seqno {
        return Err(EnvelopeError::SeqnoMismatch);
    }
    if msg.hdr.msg_type != expected_msg_type as u8 {
        return Err(EnvelopeError::MsgTypeMismatch);
    }

    let msg_size = msg.hdr.msg_size as usize;
    if msg_size > SNP_GUEST_MSG_SIZE - 96 {
        return Err(EnvelopeError::HeaderMismatch);
    }
    if plaintext_out.len() < msg_size {
        return Err(EnvelopeError::OutputBufferTooSmall);
    }

    // Rebuild the AAD from the header bytes that cover offsets
    // 0x30..0x60 (AMD spec §5.4.2).
    let mut aad = [0u8; SNP_GUEST_MSG_AAD_LEN];
    {
        let hdr_bytes = <SnpGuestMsgHdr as IntoBytes>::as_bytes(&msg.hdr);
        aad.copy_from_slice(&hdr_bytes[SNP_GUEST_MSG_AAD_OFFSET..0x60]);
    }

    // Copy the (possibly-short) ciphertext into the caller's buffer
    // so we can decrypt in place without touching `msg`.
    plaintext_out[..msg_size].copy_from_slice(&msg.payload[..msg_size]);

    let iv = SnpGuestMsg::iv_from_seqno(msg.hdr.msg_seqno);
    let cipher = Aes256Gcm::new(GenericArray::from_slice(vmpck));
    let nonce = Nonce::from_slice(&iv);
    let tag = Tag::from_slice(&msg.hdr.auth_tag[..AEAD_TAG_LEN]);

    cipher
        .decrypt_in_place_detached(nonce, &aad, &mut plaintext_out[..msg_size], tag)
        .map_err(|_| EnvelopeError::AuthenticationFailed)?;

    Ok(msg_size)
}

/// Convenience: size of a MSG_REPORT_REQ plaintext (96 B). Exposed
/// so callers allocating `plaintext_out` buffers for `encrypt_guest_msg`
/// don't have to spell the constant out.
pub const REPORT_REQ_PLAINTEXT_SIZE: usize = MSG_REPORT_REQ_SIZE;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MsgReportReq, MsgReportResp};

    /// Canonical deterministic VMPCK used by every test. Not a real
    /// key — 32 bytes of 0xA5.
    const TEST_VMPCK: [u8; VMPCK_LEN] = [0xA5; VMPCK_LEN];

    #[test]
    fn round_trip_report_req() {
        let mut rd = [0u8; 64];
        for (i, b) in rd.iter_mut().enumerate() {
            *b = i as u8;
        }
        let req = MsgReportReq::new(rd, 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);

        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(
            &mut env,
            &TEST_VMPCK,
            42,
            SnpMsgType::ReportReq,
            0,
            plaintext,
        )
        .expect("encrypt ok");

        // Ciphertext must not equal plaintext.
        assert_ne!(
            &env.payload[..plaintext.len()],
            plaintext,
            "payload should be encrypted"
        );
        // Auth tag populated.
        assert!(env.hdr.auth_tag[..AEAD_TAG_LEN].iter().any(|&b| b != 0));

        let mut decoded = [0u8; MSG_REPORT_REQ_SIZE];
        let n = decrypt_guest_msg(
            &env,
            &TEST_VMPCK,
            42,
            SnpMsgType::ReportReq,
            0,
            &mut decoded,
        )
        .expect("decrypt ok");
        assert_eq!(n, MSG_REPORT_REQ_SIZE);
        assert_eq!(&decoded[..], plaintext);
    }

    #[test]
    fn wrong_seqno_rejected() {
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);

        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(&mut env, &TEST_VMPCK, 5, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();

        let mut decoded = [0u8; MSG_REPORT_REQ_SIZE];
        assert_eq!(
            decrypt_guest_msg(&env, &TEST_VMPCK, 6, SnpMsgType::ReportReq, 0, &mut decoded),
            Err(EnvelopeError::SeqnoMismatch)
        );
    }

    #[test]
    fn wrong_msg_type_rejected() {
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);

        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(&mut env, &TEST_VMPCK, 1, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();

        let mut decoded = [0u8; MSG_REPORT_REQ_SIZE];
        assert_eq!(
            decrypt_guest_msg(&env, &TEST_VMPCK, 1, SnpMsgType::ReportResp, 0, &mut decoded),
            Err(EnvelopeError::MsgTypeMismatch)
        );
    }

    #[test]
    fn tampered_ciphertext_rejected() {
        let req = MsgReportReq::new([0xEE; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);

        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(&mut env, &TEST_VMPCK, 7, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();

        // Flip a bit in the ciphertext.
        env.payload[10] ^= 0x01;

        let mut decoded = [0u8; MSG_REPORT_REQ_SIZE];
        assert_eq!(
            decrypt_guest_msg(&env, &TEST_VMPCK, 7, SnpMsgType::ReportReq, 0, &mut decoded),
            Err(EnvelopeError::AuthenticationFailed)
        );
    }

    #[test]
    fn wrong_key_rejected() {
        let req = MsgReportReq::new([0x11; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);

        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(&mut env, &TEST_VMPCK, 1, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();

        let wrong_key = [0xBB; VMPCK_LEN];
        let mut decoded = [0u8; MSG_REPORT_REQ_SIZE];
        assert_eq!(
            decrypt_guest_msg(&env, &wrong_key, 1, SnpMsgType::ReportReq, 0, &mut decoded),
            Err(EnvelopeError::AuthenticationFailed)
        );
    }

    #[test]
    fn plaintext_too_large_rejected() {
        // Payload region is SNP_GUEST_MSG_SIZE - 96 = 4000 B.
        let oversize = [0u8; SNP_GUEST_MSG_SIZE - 96 + 1];
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        assert_eq!(
            encrypt_guest_msg(&mut env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, &oversize),
            Err(EnvelopeError::PlaintextTooLarge)
        );
    }

    #[test]
    fn output_buffer_too_small_rejected() {
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);

        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(&mut env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();

        let mut tiny = [0u8; 10];
        assert_eq!(
            decrypt_guest_msg(&env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, &mut tiny),
            Err(EnvelopeError::OutputBufferTooSmall)
        );
    }

    #[test]
    fn wrong_msg_vmpck_rejected() {
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);

        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(&mut env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();

        let mut decoded = [0u8; MSG_REPORT_REQ_SIZE];
        assert_eq!(
            decrypt_guest_msg(&env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 1, &mut decoded),
            Err(EnvelopeError::HeaderMismatch)
        );
    }

    #[test]
    fn encrypt_writes_expected_header_fields() {
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(&mut env, &TEST_VMPCK, 0xABCD_EF01, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();

        assert_eq!({ env.hdr.msg_seqno }, 0xABCD_EF01);
        assert_eq!(env.hdr.algo_id, SNP_AEAD_AES_256_GCM);
        assert_eq!(env.hdr.hdr_version, SNP_GUEST_MSG_HDR_VERSION);
        assert_eq!({ env.hdr.hdr_size }, 0x60);
        assert_eq!(env.hdr.msg_type, SnpMsgType::ReportReq as u8);
        assert_eq!(env.hdr.msg_version, SNP_REPORT_MSG_VERSION);
        assert_eq!({ env.hdr.msg_size }, MSG_REPORT_REQ_SIZE as u16);
        assert_eq!(env.hdr.msg_vmpck, 0);
    }

    #[test]
    fn short_plaintext_zero_pads_tail() {
        let small = b"hello";
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(&mut env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, small)
            .unwrap();
        // Everything past the ciphertext must stay zero.
        assert!(env.payload[small.len()..].iter().all(|&b| b == 0));

        let mut out = [0u8; 5];
        let n = decrypt_guest_msg(&env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, &mut out)
            .unwrap();
        assert_eq!(n, 5);
        assert_eq!(&out, b"hello");
    }

    #[test]
    fn report_resp_round_trip() {
        // Build a synthetic MSG_REPORT_RESP plaintext, encrypt, then
        // verify that decrypt recovers exactly the bytes we sent.
        let resp = MsgReportResp::read_from_bytes(&[0u8; 1280]).unwrap();
        let plaintext = <MsgReportResp as IntoBytes>::as_bytes(&resp);

        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(&mut env, &TEST_VMPCK, 2, SnpMsgType::ReportResp, 0, plaintext)
            .unwrap();

        let mut decoded = [0u8; 1280];
        let n = decrypt_guest_msg(&env, &TEST_VMPCK, 2, SnpMsgType::ReportResp, 0, &mut decoded)
            .unwrap();
        assert_eq!(n, 1280);
        assert_eq!(&decoded[..], plaintext);
    }
}
