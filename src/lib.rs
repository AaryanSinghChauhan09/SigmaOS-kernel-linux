#![forbid(unsafe_code)]
#![deny(clippy::all)]

//! Linux kernel ABI / syscall emulation layer for SigmaOS.

use std::fmt;

// ---------------------------------------------------------------------------
// Syscall enum
// ---------------------------------------------------------------------------

/// Linux syscall numbers (x86-64 ABI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum Syscall {
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,
    Stat = 4,
    Fstat = 5,
    Mmap = 9,
    Munmap = 11,
    Brk = 12,
    Getpid = 39,
    Socket = 41,
    Connect = 42,
    Accept = 43,
    Sendto = 44,
    Recvfrom = 45,
    Fork = 57,
    Execve = 59,
    Exit = 60,
    Wait4 = 61,
    Kill = 62,
    Getuid = 102,
}

// ---------------------------------------------------------------------------
// SyscallError
// ---------------------------------------------------------------------------

/// Errors that a syscall dispatch can return, mapping to standard errno values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyscallError {
    PermissionDenied,
    NoSuchFile,
    InvalidArgument,
    OutOfMemory,
    Interrupted,
    TryAgain,
    NotSupported,
}

impl SyscallError {
    /// Convert this error to the corresponding POSIX errno integer.
    pub fn to_errno(self) -> i32 {
        match self {
            Self::PermissionDenied => 13, // EACCES
            Self::NoSuchFile => 2,        // ENOENT
            Self::InvalidArgument => 22,  // EINVAL
            Self::OutOfMemory => 12,      // ENOMEM
            Self::Interrupted => 4,       // EINTR
            Self::TryAgain => 11,         // EAGAIN
            Self::NotSupported => 38,     // ENOSYS
        }
    }
}

impl fmt::Display for SyscallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "errno {}: {:?}", self.to_errno(), self)
    }
}

// ---------------------------------------------------------------------------
// SyscallResult
// ---------------------------------------------------------------------------

/// Return type for every dispatched syscall.
pub type SyscallResult = Result<i64, SyscallError>;

// ---------------------------------------------------------------------------
// KernelAbi
// ---------------------------------------------------------------------------

/// Represents the kernel ABI contract exposed to user-space.
#[derive(Debug)]
pub struct KernelAbi {
    /// Target architecture string.
    pub arch: &'static str,
    /// ABI version number.
    pub abi_version: u32,
    /// System page size in bytes.
    pub page_size: usize,
}

impl Default for KernelAbi {
    fn default() -> Self {
        Self {
            arch: "x86_64",
            abi_version: 5,
            page_size: 4096,
        }
    }
}

impl KernelAbi {
    /// Create a new `KernelAbi` with default x86-64 constants.
    pub fn new() -> Self {
        Self::default()
    }

    /// Dispatch a syscall with the given arguments.
    ///
    /// Returns a stub result for every recognised syscall; returns
    /// `Err(SyscallError::NotSupported)` for any unimplemented path that
    /// would require real kernel interaction.
    pub fn dispatch(&self, syscall: Syscall, args: &[u64]) -> SyscallResult {
        match syscall {
            // --- I/O ---
            Syscall::Read => {
                // args: fd, buf_ptr, count
                let count = args.get(2).copied().unwrap_or(0);
                if count == 0 {
                    Ok(0)
                } else {
                    // Emulated: no real fd table yet
                    Err(SyscallError::NotSupported)
                }
            }
            Syscall::Write => {
                // args: fd, buf_ptr, count
                let count = args.get(2).copied().unwrap_or(0) as i64;
                if count < 0 {
                    Err(SyscallError::InvalidArgument)
                } else {
                    Ok(count) // report all bytes "written"
                }
            }
            Syscall::Open => Err(SyscallError::NotSupported),
            Syscall::Close => Ok(0),

            // --- File metadata ---
            Syscall::Stat | Syscall::Fstat => Err(SyscallError::NotSupported),

            // --- Memory management ---
            Syscall::Mmap => {
                // Stub: cannot allocate real pages without unsafe
                Err(SyscallError::NotSupported)
            }
            Syscall::Munmap => Ok(0),
            Syscall::Brk => {
                // Return the requested address as the new break
                Ok(args.first().copied().unwrap_or(0) as i64)
            }

            // --- Process ---
            Syscall::Getpid => Ok(1), // PID 1 in emulation
            Syscall::Getuid => Ok(0), // root uid in emulation
            Syscall::Fork => Err(SyscallError::NotSupported),
            Syscall::Execve => Err(SyscallError::NotSupported),
            Syscall::Exit => Ok(args.first().copied().unwrap_or(0) as i64),
            Syscall::Wait4 => Err(SyscallError::NotSupported),
            Syscall::Kill => {
                let sig = args.get(1).copied().unwrap_or(0);
                if sig == 0 {
                    Ok(0)
                } else {
                    Err(SyscallError::PermissionDenied)
                }
            }

            // --- Networking ---
            Syscall::Socket => Err(SyscallError::NotSupported),
            Syscall::Connect => Err(SyscallError::NotSupported),
            Syscall::Accept => Err(SyscallError::NotSupported),
            Syscall::Sendto => Err(SyscallError::NotSupported),
            Syscall::Recvfrom => Err(SyscallError::NotSupported),
        }
    }
}

// ---------------------------------------------------------------------------
// errno helpers
// ---------------------------------------------------------------------------

/// Return a human-readable description for a POSIX errno value.
pub fn errno_to_str(errno: i32) -> &'static str {
    match errno {
        1 => "EPERM: Operation not permitted",
        2 => "ENOENT: No such file or directory",
        4 => "EINTR: Interrupted system call",
        9 => "EBADF: Bad file descriptor",
        11 => "EAGAIN: Resource temporarily unavailable",
        12 => "ENOMEM: Out of memory",
        13 => "EACCES: Permission denied",
        14 => "EFAULT: Bad address",
        17 => "EEXIST: File exists",
        22 => "EINVAL: Invalid argument",
        28 => "ENOSPC: No space left on device",
        32 => "EPIPE: Broken pipe",
        38 => "ENOSYS: Function not implemented",
        _ => "Unknown errno",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to build a default ABI instance
    fn abi() -> KernelAbi {
        KernelAbi::new()
    }

    // --- AAA Test 1: dispatch read with count=0 returns Ok(0) ---
    #[test]
    fn test_dispatch_read_zero_count() {
        // Arrange
        let kernel = abi();
        let args = [3u64, 0xDEAD_BEEF, 0]; // fd=3, buf=..., count=0

        // Act
        let result = kernel.dispatch(Syscall::Read, &args);

        // Assert
        assert_eq!(result, Ok(0));
    }

    // --- AAA Test 2: dispatch write returns byte count ---
    #[test]
    fn test_dispatch_write_returns_count() {
        // Arrange
        let kernel = abi();
        let args = [1u64, 0xDEAD_BEEF, 42]; // fd=1 (stdout), buf=..., count=42

        // Act
        let result = kernel.dispatch(Syscall::Write, &args);

        // Assert
        assert_eq!(result, Ok(42));
    }

    // --- AAA Test 3: SyscallError converts to correct errno values ---
    #[test]
    fn test_error_to_errno_values() {
        // Arrange / Act / Assert (table-driven)
        assert_eq!(SyscallError::PermissionDenied.to_errno(), 13);
        assert_eq!(SyscallError::NoSuchFile.to_errno(), 2);
        assert_eq!(SyscallError::InvalidArgument.to_errno(), 22);
        assert_eq!(SyscallError::OutOfMemory.to_errno(), 12);
        assert_eq!(SyscallError::Interrupted.to_errno(), 4);
        assert_eq!(SyscallError::TryAgain.to_errno(), 11);
        assert_eq!(SyscallError::NotSupported.to_errno(), 38);
    }

    // --- AAA Test 4: KernelAbi constants are correct ---
    #[test]
    fn test_kernel_abi_constants() {
        // Arrange
        let kernel = abi();

        // Act + Assert
        assert_eq!(kernel.arch, "x86_64");
        assert_eq!(kernel.abi_version, 5);
        assert_eq!(kernel.page_size, 4096);
    }

    // --- AAA Test 5: errno_to_str maps known values ---
    #[test]
    fn test_errno_to_str_known() {
        // Arrange / Act / Assert
        assert!(errno_to_str(2).contains("ENOENT"));
        assert!(errno_to_str(12).contains("ENOMEM"));
        assert!(errno_to_str(22).contains("EINVAL"));
        assert!(errno_to_str(13).contains("EACCES"));
    }

    // --- AAA Test 6: errno_to_str returns fallback for unknown ---
    #[test]
    fn test_errno_to_str_unknown() {
        // Arrange
        let unknown = 9999;

        // Act
        let description = errno_to_str(unknown);

        // Assert
        assert_eq!(description, "Unknown errno");
    }

    // --- AAA Test 7: Syscall enum numeric values are correct ---
    #[test]
    fn test_syscall_numbers() {
        // Arrange / Act / Assert
        assert_eq!(Syscall::Read as u64, 0);
        assert_eq!(Syscall::Write as u64, 1);
        assert_eq!(Syscall::Open as u64, 2);
        assert_eq!(Syscall::Exit as u64, 60);
        assert_eq!(Syscall::Getuid as u64, 102);
        assert_eq!(Syscall::Fork as u64, 57);
        assert_eq!(Syscall::Getpid as u64, 39);
    }
}
