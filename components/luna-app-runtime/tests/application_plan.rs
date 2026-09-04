#[cfg(unix)]
mod unix {
    use luna_app_runtime::application_plan_runtime::ApplicationPlanLauncher;
    use luna_app_runtime::LinuxApplicationRuntime;

    #[test]
    fn linux_runtime_exposes_authorized_plan_launcher() {
        fn assert_launcher<T: ApplicationPlanLauncher>() {}
        assert_launcher::<LinuxApplicationRuntime>();
    }
}
