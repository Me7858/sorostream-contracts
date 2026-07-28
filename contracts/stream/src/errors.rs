use soroban_sdk::contracterror;

/// Custom errors for the SoroStream contract.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StreamError {
    StreamNotFound = 1,
    NotRecipient = 2,
    NotSender = 3,
    StreamNotActive = 4,
    ZeroAmount = 5,
    InvalidDuration = 6,
    InsufficientBalance = 7,
    InvalidCliff = 8,
    AlreadyInitialized = 9,
    NotInitialized = 10,
    DuplicateStream = 11,
    InvalidStartTime = 12,
    InvalidPartialCancel = 13,
    ContractPaused = 14,
    Overflow = 15,
    ZeroFlowRate = 16,
    BatchLengthMismatch = 17,
    TokenMismatch = 18,
    StreamLocked = 19,
    NotAuthorized = 20,
    StreamNotPaused = 21,
    StreamDurationTooShort = 22,
    StreamIdConflict = 23,
    SenderStreamLimitExceeded = 24,
    InvalidNonce = 25,
    MigrationAlreadyApplied = 26,
    StreamNotSettled = 27,
    WithdrawalCooldownActive = 28,
    RecipientNotWhitelisted = 29,
    MetadataTooLong = 30,
    InvalidEndTime = 31,
    InsufficientXlmForFee = 32,
    DuplicateStreamId = 33,
    ReentrancyDetected = 34,
    InvalidMetadataUri = 35,
    StreamNotComplete = 36,
    TokenNotWhitelisted = 37,
    /// One or more tranches have invalid data (e.g. zero amount, unsorted unlock times,
    /// total tranche amount does not match deposit, or empty tranche list on step-vesting).
    InvalidTranches = 38,
    /// Oracle price deviates from the creation price by more than `max_price_deviation_bps`.
    PriceDeviationTooHigh = 39,
    /// Oracle contract call failed or returned an unexpected value.
    OracleError = 39,
    RateLimitExceeded = 37,
    TokenNotWhitelisted = 38,
    SlippageExceeded = 39,
    InvalidSlippage = 40,
    DurationExceedsMax = 41,
    InvalidTokenAddress = 42,
    /// `start_time` is further in the future than the admin-configured
    /// `max_future_start_offset_seconds` (default: 365 days).
    StartTimeTooFar = 43,
    OracleError = 40,
    RateLimitExceeded = 41,
    SlippageExceeded = 42,
    InvalidSlippage = 43,
    DurationExceedsMax = 44,
    InvalidTokenAddress = 45,
    /// Stream ID derived from (sender, recipient, start_time, nonce) collided with an
    /// existing entry even after the defensive retry increment.  This should never occur
    /// in normal operation; it indicates a SHA-256 prefix collision or a storage bug.
    IDCollision = 46,
    /// A withdrawal was attempted before the next evenly-spaced step threshold has
    /// been reached.  Callers should wait until
    /// `start_time + (current_step + 1) * step_interval` passes.
    NextStepNotReached = 47,
    /// The claimable amount is below the stream's configured `min_withdrawal_amount`
    /// floor.  This check is bypassed on the final claim so the recipient can always
    /// drain the remaining balance.
    AmountBelowMinimum = 48,

    // ── Feature (a): StreamExpiryWarning ─────────────────────────────────────
    /// Expiry warning window must be a positive ledger count.
    InvalidExpiryWindow = 46,

    // ── Feature (b): Sender reputation cap ───────────────────────────────────
    /// Sender is below the promotion threshold and has hit the new-sender stream cap.
    NewSenderStreamCapExceeded = 47,

    // ── Feature (c): Stream redirect ─────────────────────────────────────────
    /// The redirect target stream does not exist or its recipient doesn't match.
    InvalidRedirectTarget = 48,
    /// Setting this redirect would create a circular chain (A→B→A or longer).
    CircularRedirect = 49,
    /// Redirect target stream must have the same recipient as the source stream.
    RedirectRecipientMismatch = 50,

    // ── Feature (d): Dual-token streams ──────────────────────────────────────
    /// Dual stream requires both token addresses to be distinct.
    DuplicateTokenInDualStream = 51,
    /// Operation requires a dual-token stream but the stream only has one token.
    NotDualStream = 52,
    /// Operation requires a single-token stream but the stream is dual-token.
    IsDualStream = 53,
    /// Sender has exceeded the allowed stream creation rate.
    RateLimitExceeded = 41,
    /// Withdrawal or operation would exceed slippage tolerance.
    SlippageExceeded = 42,
    /// The provided slippage parameter is invalid (e.g. > 10 000 bps).
    InvalidSlippage = 43,
    /// Stream duration exceeds the configured maximum.
    DurationExceedsMax = 44,
    /// Token address is not a valid deployed SAC.
    InvalidTokenAddress = 45,
}
