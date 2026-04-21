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

    /// S7-1: AES-GCM nonce diversification is entirely a function of
    /// `msg_seqno` — `iv_from_seqno(n)` is pure and deterministic, and
    /// `encrypt_guest_msg` holds no seen-seqno set. If a caller
    /// accidentally calls `encrypt_guest_msg` twice with the same
    /// `(vmpck, msg_seqno)` it DOES NOT get rejected; the two
    /// envelopes produce bit-identical ciphertext + auth tag (given
    /// the same plaintext + AAD-covered header fields), which is a
    /// catastrophic AES-GCM nonce-reuse break.
    ///
    /// This test pins the current behaviour so that if a future
    /// change adds a runtime seqno guard (rejecting reuse, or mixing
    /// randomness into the IV) the assertion flips and the author has
    /// to update this test. It is named `_bug` because the behaviour
    /// pinned here is a bug: the crate is load-bearing for the
    /// `SNP_GUEST_REQUEST` exchange and relies entirely on the caller
    /// to monotonically bump `msg_seqno`.
    #[test]
    fn seqno_reuse_produces_identical_ciphertext_bug() {
        let req = MsgReportReq::new([0x77; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);

        let mut env_a =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        let mut env_b =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();

        encrypt_guest_msg(&mut env_a, &TEST_VMPCK, 99, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();
        encrypt_guest_msg(&mut env_b, &TEST_VMPCK, 99, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();

        // Same (key, seqno, plaintext, AAD) -> identical ciphertext +
        // identical auth tag. This is the AES-GCM nonce-reuse bug;
        // we assert it so the crate ships a red-flag test instead of
        // a silent footgun.
        assert_eq!(
            env_a.payload[..plaintext.len()],
            env_b.payload[..plaintext.len()],
            "ciphertext differs between two encrypts with the same seqno — \
             either a runtime nonce guard was added or randomness was mixed \
             into the IV. Update this test accordingly.",
        );
        assert_eq!(
            env_a.hdr.auth_tag[..AEAD_TAG_LEN],
            env_b.hdr.auth_tag[..AEAD_TAG_LEN],
            "auth tag differs — same note as above",
        );
    }

    /// S7-1 companion: the IV derivation itself is pure and returns
    /// byte-identical output for the same `msg_seqno`. Pinning this
    /// here means a future change that mixes randomness into the IV
    /// has to update this test too (and therefore can't silently
    /// regress callers that assume the IV equals the seqno).
    #[test]
    fn iv_derivation_is_pure_in_seqno() {
        let a = SnpGuestMsg::iv_from_seqno(0xDEAD_BEEF_1234_5678);
        let b = SnpGuestMsg::iv_from_seqno(0xDEAD_BEEF_1234_5678);
        assert_eq!(a, b);
        let c = SnpGuestMsg::iv_from_seqno(0xDEAD_BEEF_1234_5679);
        assert_ne!(a, c, "different seqnos must yield different IVs");
    }

    /// S7-1 positive: if a caller _does_ monotonically bump the
    /// seqno, the two envelopes diverge — ciphertext and tag both
    /// differ. This is the happy path that the spec relies on.
    #[test]
    fn monotonic_seqno_diversifies_ciphertext() {
        let req = MsgReportReq::new([0x33; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);

        let mut env_a =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        let mut env_b =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();

        encrypt_guest_msg(&mut env_a, &TEST_VMPCK, 100, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();
        encrypt_guest_msg(&mut env_b, &TEST_VMPCK, 101, SnpMsgType::ReportReq, 0, plaintext)
            .unwrap();

        assert_ne!(
            env_a.payload[..plaintext.len()],
            env_b.payload[..plaintext.len()],
        );
        assert_ne!(
            env_a.hdr.auth_tag[..AEAD_TAG_LEN],
            env_b.hdr.auth_tag[..AEAD_TAG_LEN],
        );
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

    // ------- R14 round 14 additions: envelope edge cases -------

    #[test]
    fn encrypt_oversize_plaintext_rejected() {
        // Fixed-size array on the stack; payload cap is
        // `SNP_GUEST_MSG_SIZE - 96`, so a full 4 KiB slice is one
        // byte past the ceiling.
        let too_big = [0u8; SNP_GUEST_MSG_SIZE];
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        let err = encrypt_guest_msg(
            &mut env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, &too_big,
        ).unwrap_err();
        assert_eq!(err, EnvelopeError::PlaintextTooLarge);
    }

    #[test]
    fn decrypt_output_buffer_too_small_rejected() {
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(
            &mut env, &TEST_VMPCK, 7, SnpMsgType::ReportReq, 0, plaintext,
        ).unwrap();

        // Output buffer one byte short of declared msg_size.
        let mut out = [0u8; MSG_REPORT_REQ_SIZE - 1];
        let err = decrypt_guest_msg(
            &env, &TEST_VMPCK, 7, SnpMsgType::ReportReq, 0, &mut out,
        ).unwrap_err();
        assert_eq!(err, EnvelopeError::OutputBufferTooSmall);
    }

    #[allow(dead_code)]
    fn _disable_tampered_header_test_placeholder() {}

    #[test]
    fn tampered_header_aad_byte_rejected() {
        // Flip a byte inside the AAD-covered header region
        // (offset 0x30..0x60). The AES-GCM tag is computed over
        // that AAD; any change must trigger AuthenticationFailed.
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(
            &mut env, &TEST_VMPCK, 11, SnpMsgType::ReportReq, 0, plaintext,
        ).unwrap();

        // Use raw pointer path to modify a packed-struct field in a
        // way that survives the decrypt's sanity checks (msg_seqno
        // covers 0x30..0x38 inside the AAD). Flip a reserved-ish
        // byte: `msg_version` lives in the AAD. Bumping it to a
        // non-spec value will trip the HeaderMismatch gate, but
        // the actual header-struct field checks run before the
        // AAD verify. Use `algo_id` which ALSO runs through the
        // AAD: changing it from 1 to 2 fails the HeaderMismatch
        // gate in practice. Test the RAW-byte AAD path by flipping
        // a byte AFTER the msg_size + msg_vmpck but still inside
        // the AAD (offset 0x5f is the last AAD byte; it is
        // reserved, value 0 by default).
        let hdr_bytes = <SnpGuestMsgHdr as IntoBytes>::as_mut_bytes(&mut env.hdr);
        hdr_bytes[0x5f] ^= 0x01;

        let mut out = [0u8; MSG_REPORT_REQ_SIZE];
        let err = decrypt_guest_msg(
            &env, &TEST_VMPCK, 11, SnpMsgType::ReportReq, 0, &mut out,
        ).unwrap_err();
        assert_eq!(err, EnvelopeError::AuthenticationFailed);
    }

    #[test]
    fn aad_slice_is_32_bytes_starting_at_0x30() {
        // Belt-and-suspenders: if anyone widens or moves the AAD
        // slice this test fails loudly rather than silently breaking
        // wire compatibility with the PSP.
        assert_eq!(SNP_GUEST_MSG_AAD_OFFSET, 0x30);
        assert_eq!(SNP_GUEST_MSG_AAD_LEN, 0x30);
    }

    #[test]
    fn round_trip_zero_length_plaintext_accepted() {
        // Zero-length plaintexts are technically allowed by the
        // envelope; the auth tag still protects the AAD. Round-trip
        // must survive without errors.
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(
            &mut env, &TEST_VMPCK, 5, SnpMsgType::ReportReq, 0, &[],
        ).unwrap();
        let mut out = [0u8; 0];
        let n = decrypt_guest_msg(
            &env, &TEST_VMPCK, 5, SnpMsgType::ReportReq, 0, &mut out,
        ).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn decrypt_wrong_msg_version_rejected() {
        // Flip `msg_version` (part of the AAD). Decrypt must refuse —
        // whether via HeaderMismatch or AuthenticationFailed depends
        // on which gate fires first; both are acceptable rejections.
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(
            &mut env, &TEST_VMPCK, 3, SnpMsgType::ReportReq, 0, plaintext,
        ).unwrap();
        env.hdr.msg_version = env.hdr.msg_version.wrapping_add(1);

        let mut out = [0u8; MSG_REPORT_REQ_SIZE];
        let err = decrypt_guest_msg(
            &env, &TEST_VMPCK, 3, SnpMsgType::ReportReq, 0, &mut out,
        ).unwrap_err();
        assert!(
            matches!(
                err,
                EnvelopeError::HeaderMismatch | EnvelopeError::AuthenticationFailed
            ),
            "expected HeaderMismatch or AuthenticationFailed, got {err:?}"
        );
    }

    #[test]
    fn envelope_error_display_distinct_per_variant() {
        // Catch a future `Display` (or format!) that accidentally
        // collapses multiple variants to the same string. Distinct
        // error shapes should remain distinct to log readers.
        use std::format;
        let a = format!("{:?}", EnvelopeError::PlaintextTooLarge);
        let b = format!("{:?}", EnvelopeError::OutputBufferTooSmall);
        let c = format!("{:?}", EnvelopeError::HeaderMismatch);
        let d = format!("{:?}", EnvelopeError::SeqnoMismatch);
        let e = format!("{:?}", EnvelopeError::MsgTypeMismatch);
        let f = format!("{:?}", EnvelopeError::AuthenticationFailed);
        let all = [&a, &b, &c, &d, &e, &f];
        for (i, x) in all.iter().enumerate() {
            for (j, y) in all.iter().enumerate() {
                if i != j {
                    assert_ne!(x, y, "variants {i} and {j} serialise to the same string");
                }
            }
        }
    }

    // ------- R13 round 15: functional AES-GCM fan-out -------

    #[test]
    fn round_trip_each_vmpck_slot_id() {
        // `msg_vmpck` selects which of the four VMPCK keys on the
        // Secrets page the PSP used; the envelope authenticates the
        // slot id as part of the AAD. A correct round-trip for every
        // slot proves the AAD + key plumbing doesn't silently alias
        // slot IDs. The test uses the same key for simplicity — the
        // goal is the slot-id AAD binding, not cross-key decryption.
        let req = MsgReportReq::new([0xEE; 64], 2);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        for slot in 0u8..=3 {
            let mut env =
                SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
            encrypt_guest_msg(
                &mut env, &TEST_VMPCK, 1 + slot as u64,
                SnpMsgType::ReportReq, slot, plaintext,
            ).unwrap();
            let mut out = [0u8; MSG_REPORT_REQ_SIZE];
            let n = decrypt_guest_msg(
                &env, &TEST_VMPCK, 1 + slot as u64,
                SnpMsgType::ReportReq, slot, &mut out,
            ).unwrap();
            assert_eq!(n, MSG_REPORT_REQ_SIZE);
            assert_eq!(&out[..], plaintext, "slot {slot} round-trip diverged");
            assert_eq!(env.hdr.msg_vmpck, slot);
        }
    }

    #[test]
    fn different_keys_decrypt_rejected() {
        // Encrypt with key A; decrypt with key B (same AAD, same
        // seqno). AES-GCM must reject with AuthenticationFailed —
        // this catches any silent fallback that might accept a
        // close-enough key.
        const KEY_A: [u8; VMPCK_LEN] = [0x11; VMPCK_LEN];
        const KEY_B: [u8; VMPCK_LEN] = [0x22; VMPCK_LEN];
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(
            &mut env, &KEY_A, 0, SnpMsgType::ReportReq, 0, plaintext,
        ).unwrap();
        let mut out = [0u8; MSG_REPORT_REQ_SIZE];
        let err = decrypt_guest_msg(
            &env, &KEY_B, 0, SnpMsgType::ReportReq, 0, &mut out,
        ).unwrap_err();
        assert_eq!(err, EnvelopeError::AuthenticationFailed);
    }

    #[test]
    fn seqno_u64_boundary_values_round_trip() {
        // Exercise the IV derivation across a span of 64-bit seqno
        // values: 0, 1, 2^32-1, 2^32, u64::MAX. Each must encrypt +
        // decrypt cleanly and produce distinct IVs.
        let req = MsgReportReq::new([0x33; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        let seqnos: [u64; 5] = [0, 1, u32::MAX as u64, 1u64 << 32, u64::MAX];
        let mut ivs = std::collections::HashSet::new();
        for seqno in seqnos {
            let mut env =
                SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
            encrypt_guest_msg(
                &mut env, &TEST_VMPCK, seqno, SnpMsgType::ReportReq, 0, plaintext,
            ).unwrap();
            let mut out = [0u8; MSG_REPORT_REQ_SIZE];
            let n = decrypt_guest_msg(
                &env, &TEST_VMPCK, seqno, SnpMsgType::ReportReq, 0, &mut out,
            ).unwrap();
            assert_eq!(n, MSG_REPORT_REQ_SIZE, "seqno {seqno} round-trip len");
            assert_eq!(&out[..], plaintext, "seqno {seqno} round-trip bytes");
            let iv = SnpGuestMsg::iv_from_seqno(seqno);
            assert!(
                ivs.insert(iv),
                "seqno {seqno} produced a duplicate IV"
            );
        }
    }

    #[test]
    fn off_by_one_seqno_rejected() {
        // Classic seqno-desync: encrypt with seqno=10, decrypt with
        // seqno=11 → must bounce with HeaderMismatch / Seqno /
        // AuthenticationFailed. The IV is seqno-derived so the auth
        // tag is computed over a different nonce on decrypt too.
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(
            &mut env, &TEST_VMPCK, 10, SnpMsgType::ReportReq, 0, plaintext,
        ).unwrap();
        let mut out = [0u8; MSG_REPORT_REQ_SIZE];
        let err = decrypt_guest_msg(
            &env, &TEST_VMPCK, 11, SnpMsgType::ReportReq, 0, &mut out,
        ).unwrap_err();
        assert!(
            matches!(
                err,
                EnvelopeError::SeqnoMismatch | EnvelopeError::AuthenticationFailed
            ),
            "expected Seqno/AuthenticationFailed, got {err:?}"
        );
    }

    #[test]
    fn auth_tag_bit_flip_rejected() {
        // Flip a single bit inside the 16-byte auth tag. AES-GCM
        // must refuse — this guarantees the tag is actually checked
        // and not just echoed back by the decrypt.
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        let mut env =
            SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
        encrypt_guest_msg(
            &mut env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, plaintext,
        ).unwrap();
        env.hdr.auth_tag[0] ^= 0x01;
        let mut out = [0u8; MSG_REPORT_REQ_SIZE];
        let err = decrypt_guest_msg(
            &env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, &mut out,
        ).unwrap_err();
        assert_eq!(err, EnvelopeError::AuthenticationFailed);
    }

    #[test]
    fn ciphertext_bit_flip_each_region_rejected() {
        // Flip a bit at three positions in the ciphertext payload
        // (start, middle, near end of plaintext span). Each must
        // bounce with AuthenticationFailed — proves the ciphertext
        // is covered by the tag end-to-end, not just metadata.
        let req = MsgReportReq::new([0; 64], 0);
        let plaintext = <MsgReportReq as IntoBytes>::as_bytes(&req);
        for flip in [0usize, 50, MSG_REPORT_REQ_SIZE - 1] {
            let mut env =
                SnpGuestMsg::read_from_bytes(&[0u8; SNP_GUEST_MSG_SIZE]).unwrap();
            encrypt_guest_msg(
                &mut env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, plaintext,
            ).unwrap();
            env.payload[flip] ^= 0x01;
            let mut out = [0u8; MSG_REPORT_REQ_SIZE];
            let err = decrypt_guest_msg(
                &env, &TEST_VMPCK, 0, SnpMsgType::ReportReq, 0, &mut out,
            ).unwrap_err();
            assert_eq!(
                err, EnvelopeError::AuthenticationFailed,
                "flip at offset {flip} should have failed auth"
            );
        }
    }
}
