pub mod api;
pub mod core;
mod frb_generated;

pub fn init_logging() {
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Debug)
                .with_tag("cook_lib_exp_rust"),
        );
    }

    #[cfg(not(target_os = "android"))]
    {
        // env_logger removed during Sherpa migration
        // logging handled by android_logger on Android
        // can add simple_logger or env_logger if needed
    }
}
