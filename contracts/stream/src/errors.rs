use soroban_sdk::contracterror;

/// Custom errors for the SoroStream contract (≤50 variants for Soroban XDR).
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
    InvalidCliff = 8,
    AlreadyInitialized = 9,
    NotInitialized = 10,
    DuplicateStream = 11,
    ContractPaused = 14,
    Overflow = 15,
    ZeroFlowRate = 16,
    BatchLengthMismatch = 17,
    TokenMismatch = 18,
    StreamLocked = 19,
    NotAuthorized = 20,
    StreamNotPaused = 21,
    StreamDurationTooShort = 22,
    InvalidNonce = 25,
    MigrationAlreadyApplied = 26,
    StreamNotSettled = 27,
    WithdrawalCooldownActive = 28,
    RecipientNotWhitelisted = 29,
    InvalidEndTime = 31,
    ReentrancyDetected = 34,
    InvalidMetadataUri = 35,
    StreamNotComplete = 36,
    TokenNotWhitelisted = 37,
    InvalidTranches = 38,
    RateLimitExceeded = 41,
    InvalidSlippage = 43,
    DurationExceedsMax = 44,
    StartTimeTooFar = 46,
    IDCollision = 47,
    NextStepNotReached = 48,
    AmountBelowMinimum = 49,
    InvalidExpiryWindow = 50,
    NewSenderStreamCapExceeded = 51,
    InvalidRedirectTarget = 52,
    CircularRedirect = 53,
    RedirectRecipientMismatch = 54,
    /// `transfer_recipient` was called on a stream marked as non-transferable at creation.
    StreamNonTransferable = 58,
    /// `withdraw` was called on a stream still in `PendingApproval` state.
    /// The recipient must call `approve_stream` first.
    AwaitingApproval = 59,
    /// `cancel_stream` was called by the sender on a stream they have irrevocably
    /// locked via `lock_stream`.
    StreamIsLocked = 60,
    /// Recipient is not on the admin-managed recipient allowlist, and the stream
    /// requires allowlist enforcement.
    RecipientNotAllowed = 61,
    MaxDepositExceeded = 64,
    /// `lock_stream_for_dispute` (or an action it guards) was called while a
    /// dispute lock is still active on the stream.
    DisputeLockActive = 69,
    /// `set_recipient_hook` was called with an invalid configuration, or the
    /// hook could not be registered (e.g. after the first withdrawal already occurred).
    InvalidRecipientHook = 70,
    /// A checkpoint was requested before the configured checkpoint interval
    /// has elapsed since the stream's last recorded checkpoint.
    CheckpointIntervalNotElapsed = 71,
}
