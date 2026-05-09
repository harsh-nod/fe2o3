pub use fe2o3_core::{KernelParams, LaunchConfig};

#[macro_export]
macro_rules! launch {
    (
        kernel: $kernel:ident,
        stream: $stream:expr,
        module: $module:expr,
        config: $config:expr,
        args: [$($kind:ident($value:expr)),* $(,)?]
    ) => {{
        let __fe2o3_function = ($module).load_function(stringify!($kernel))?;
        let mut __fe2o3_params = ::fe2o3_core::KernelParams::new();
        $(
            $crate::__push_kernel_arg!(__fe2o3_params, $kind($value));
        )*
        unsafe {
            ::fe2o3_core::launch_kernel_on_stream(
                &__fe2o3_function,
                $config,
                &$stream,
                &mut __fe2o3_params,
            )
        }
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __push_kernel_arg {
    ($params:ident, scalar($value:expr)) => {{
        $params.push($value);
    }};
    ($params:ident, raw($value:expr)) => {{
        $params.push($value);
    }};
    ($params:ident, buffer($value:expr)) => {{
        $params.push(($value).as_device_ptr());
    }};
    ($params:ident, slice($value:expr)) => {{
        $params.push(($value).as_device_ptr());
        $params.push(($value).len());
    }};
    ($params:ident, slice_mut($value:expr)) => {{
        $params.push(($value).as_device_ptr());
        $params.push(($value).len());
    }};
}
