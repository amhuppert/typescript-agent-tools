function recover(value: string | undefined) {
    if (value) {
        const narrowed = value;
        interface Broken<T> { item: T; evidence: typeof narrowed; }
        interface FailedSource { <T>(input: Broken<T>): T; }
        interface FailedTarget { (input: Broken<number>): number; }
        let failedSource!: FailedSource;
        interface FailedChooser {
            (input: FailedTarget): "chosen";
            (input: FailedSource): "same";
        }
        let chooseFailed!: FailedChooser;
        const failedResult = chooseFailed(failedSource);
        interface HealthySource { <T>(input: { item: T }): T; }
        interface HealthyTarget { (input: { item: number }): number; }
        let healthySource!: HealthySource;
        interface HealthyChooser {
            (input: HealthyTarget): "chosen";
            (input: HealthySource): "same";
        }
        let chooseHealthy!: HealthyChooser;
        const healthyResult = chooseHealthy(healthySource);
        let recoveryEnvelope: { failed: typeof failedResult; healthy: typeof healthyResult; repeated: typeof failedResult; };
    }
}
