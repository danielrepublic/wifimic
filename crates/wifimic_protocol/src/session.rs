/// A failure returned when a process has exhausted all representable IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionIdError {
    /// No strictly greater `u64` can be issued after `u64::MAX`.
    Exhausted,
}

impl std::fmt::Display for SessionIdError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exhausted => formatter.write_str("session ID space exhausted"),
        }
    }
}

impl std::error::Error for SessionIdError {}

/// Returns whether a candidate session strictly supersedes the high-water mark.
#[must_use]
pub const fn is_newer_session(last_accepted: Option<u64>, candidate: u64) -> bool {
    match last_accepted {
        None => true,
        Some(last) => candidate > last,
    }
}

/// Alias expressing the acceptance rule at a protocol endpoint.
#[must_use]
pub const fn accepts_session_id(last_accepted: Option<u64>, candidate: u64) -> bool {
    is_newer_session(last_accepted, candidate)
}

/// Computes the next strictly increasing session ID from an injected clock value.
///
/// The caller supplies the current Unix epoch millisecond value, which keeps the
/// clock seam deterministic for tests. The result is
/// `max(current_unix_epoch_ms, last_issued_session_id + 1)`.
pub const fn next_session_id(
    current_unix_epoch_ms: u64,
    last_issued_session_id: u64,
) -> Result<u64, SessionIdError> {
    let Some(after_last) = last_issued_session_id.checked_add(1) else {
        return Err(SessionIdError::Exhausted);
    };
    let next = if current_unix_epoch_ms > after_last {
        current_unix_epoch_ms
    } else {
        after_last
    };
    Ok(next)
}

/// In-process monotonic session ID generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionIdGenerator {
    last_issued_session_id: u64,
}

impl SessionIdGenerator {
    /// Creates a generator with the plan's required zero high-water mark.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_issued_session_id: 0,
        }
    }

    /// Issues the next ID using a caller-provided current epoch millisecond value.
    ///
    /// # Errors
    ///
    /// Returns [`SessionIdError::Exhausted`] only after `u64::MAX` has already
    /// been issued, when strict increase is no longer representable.
    pub fn next_id(&mut self, current_unix_epoch_ms: u64) -> Result<u64, SessionIdError> {
        let next = next_session_id(current_unix_epoch_ms, self.last_issued_session_id)?;
        self.last_issued_session_id = next;
        Ok(next)
    }

    /// Returns the latest ID issued by this generator.
    #[must_use]
    pub const fn last_issued(&self) -> u64 {
        self.last_issued_session_id
    }
}

impl Default for SessionIdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Endpoint-local high-water mark for strict session acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SessionOrder {
    last_accepted: Option<u64>,
}

impl SessionOrder {
    /// Creates an empty ordering primitive that accepts its first session.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_accepted: None,
        }
    }

    /// Accepts and records only a strictly newer session ID.
    pub fn accept(&mut self, candidate: u64) -> bool {
        if is_newer_session(self.last_accepted, candidate) {
            self.last_accepted = Some(candidate);
            true
        } else {
            false
        }
    }

    /// Returns the endpoint's current session high-water mark.
    #[must_use]
    pub const fn last_accepted(&self) -> Option<u64> {
        self.last_accepted
    }
}
