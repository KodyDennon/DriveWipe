use drivewipe_core::time_estimate::TimeEstimator;

#[test]
fn test_time_estimator_basic() {
    let total_bytes: u64 = 1_000_000_000; // 1 GB
    let mut estimator = TimeEstimator::new(total_bytes, 1, false);

    // Feed throughput samples — only complete 50% so remaining > 0
    for i in 0..10u64 {
        let bytes_done = (i + 1) * 50_000_000; // 50MB chunks, up to 500MB
        estimator.update(100_000_000.0, bytes_done, 1); // 100 MB/s
    }

    let estimate = estimator.estimate();
    assert!(estimate.calibrated);
    // 500MB remaining at 100MB/s = ~5 seconds expected
    assert!(estimate.expected_secs > 0.0);
    assert!(estimate.expected_secs < 30.0); // generous bound
}

#[test]
fn test_time_estimator_calibration_period() {
    let mut estimator = TimeEstimator::new(1_000_000_000, 1, false);

    // During calibration (first 5 samples), estimate may not be calibrated
    estimator.update(100_000_000.0, 50_000_000, 1);
    let est = estimator.estimate();
    // After just 1 sample, calibrated should be false
    assert!(!est.calibrated);
}

#[test]
fn test_time_estimator_ema_smoothing() {
    let mut estimator = TimeEstimator::new(1_000_000_000, 1, false);

    // Feed consistent throughput
    for i in 0..10u64 {
        estimator.update(100_000_000.0, (i + 1) * 10_000_000, 1);
    }

    let est1 = estimator.estimate();
    let throughput1 = est1.throughput_bps;

    // Feed a spike
    estimator.update(500_000_000.0, 200_000_000, 1);

    let est2 = estimator.estimate();
    let throughput2 = est2.throughput_bps;

    // EMA should dampen the spike
    assert!(throughput2 > throughput1);
    assert!(throughput2 < 500_000_000.0);
}

#[test]
fn test_time_estimator_multi_pass() {
    let mut estimator = TimeEstimator::new(1_000_000_000, 3, false);

    for i in 0..20u64 {
        estimator.update(100_000_000.0, (i + 1) * 50_000_000, 1);
    }

    let est = estimator.estimate();
    // Multi-pass should give an estimate
    assert!(est.expected_secs > 0.0);
}
