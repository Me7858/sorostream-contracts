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
}
