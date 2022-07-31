/// Returns HRESULT and logs the error in log file.
#[macro_export]
macro_rules! xfs_reject {
    ($l:expr) => {{
        tracing::error!(stringify!($l));
        return $l;
    }};
}

/// Returns Err(HRESULT) and logs the error in log file.
#[macro_export]
macro_rules! xfs_reject_err {
    ($l:expr) => {{
        tracing::error!(stringify!($l));
        return Err($l);
    }};
}

/// Unwraps the result of a WFS call and returns the HRESULT on error.
/// The error is logged to the log file.
#[macro_export]
macro_rules! xfs_unwrap {
    ($l:expr) => {
        match $l {
            Ok(result) => result,
            Err(error) => {
                tracing::error!("{:?}", error);
                return WFS_ERR_INTERNAL_ERROR;
            }
        }
    };
}

/// Unwraps the result of a WFS call and returns the HRESULT on error.
/// The error is logged to the log file.
#[macro_export]
macro_rules! xfs_unwrap_err {
    ($l:expr) => {
        match $l {
            Ok(result) => result,
            Err(error) => {
                tracing::error!("{:?}", error);
                return Err(WFS_ERR_INTERNAL_ERROR);
            }
        }
    };
}
