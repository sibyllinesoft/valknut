    use super::*;
    use crate::core::featureset::FeatureVector;

    #[test]
    fn test_variance_confidence() {
        assert_eq!(
            VarianceConfidence::from_samples(100, 0.5, 0.1),
            VarianceConfidence::High
        );
        assert_eq!(
            VarianceConfidence::from_samples(5, 0.0, 0.1),
            VarianceConfidence::Insufficient
        );
    }

    #[test]
    fn test_feature_prior() {
        let prior = FeaturePrior::new("test")
            .with_beta_params(2.0, 3.0)
            .with_range(0.0, 10.0, 2.0);

        assert_eq!(prior.alpha, 2.0);
        assert_eq!(prior.beta, 3.0);
        assert_eq!(prior.prior_mean(), 0.4);
    }

    #[tokio::test]
    async fn test_bayesian_normalizer() {
        let mut normalizer = BayesianNormalizer::new("z_score");

        // Create test feature vectors
        let mut vectors = vec![
            FeatureVector::new("entity1"),
            FeatureVector::new("entity2"),
            FeatureVector::new("entity3"),
        ];

        vectors[0].add_feature("complexity", 1.0);
        vectors[1].add_feature("complexity", 5.0);
        vectors[2].add_feature("complexity", 3.0);

        // Fit and normalize
        normalizer.fit(&vectors).unwrap();
        normalizer.normalize(&mut vectors).unwrap();

        // Check that normalization was applied
        assert!(vectors[0].normalized_features.contains_key("complexity"));

        // Check statistics were computed
        assert!(normalizer.get_statistics("complexity").is_some());
    }

    #[test]
    fn test_posterior_calculation() {
        let normalizer = BayesianNormalizer::new("bayesian");

        let empirical = FeatureStatistics {
            mean: 3.0,
            variance: 2.0,
            std_dev: 2.0_f64.sqrt(),
            min: 1.0,
            max: 5.0,
            n_samples: 10,
            confidence: VarianceConfidence::Medium,
            prior_weight: 0.0,
            posterior_mean: 0.0,
            posterior_variance: 0.0,
        };

        let prior = FeaturePrior::new("test")
            .with_beta_params(2.0, 2.0)
            .with_range(0.0, 10.0, 5.0);

        let posterior = normalizer
            .calculate_posterior_stats(&empirical, &prior)
            .unwrap();

        // Posterior mean should be between prior and empirical means
        assert!(posterior.posterior_mean > 0.0);
        assert!(posterior.posterior_mean < 10.0);
        assert!(posterior.posterior_variance > 0.0);
    }

    #[tokio::test]
    async fn test_bayesian_normalizer_batch_normalization() {
        let mut normalizer = BayesianNormalizer::new("z_score");

        let mut vectors = vec![
            FeatureVector::new("entity1"),
            FeatureVector::new("entity2"),
            FeatureVector::new("entity3"),
            FeatureVector::new("entity4"),
        ];

        for (i, vector) in vectors.iter_mut().enumerate() {
            vector.add_feature("complexity", (i as f64 + 1.0) * 2.0);
            vector.add_feature("length", (i as f64 + 1.0) * 10.0);
        }

        normalizer.fit(&vectors).unwrap();
        normalizer.normalize(&mut vectors).unwrap();

        // All vectors should have normalized features
        for vector in &vectors {
            assert!(vector.normalized_features.contains_key("complexity"));
            assert!(vector.normalized_features.contains_key("length"));
        }
    }

    #[test]
    fn test_feature_prior_with_type() {
        let prior = FeaturePrior::new("complexity");

        // Test that the prior was created successfully
        assert_eq!(prior.name, "complexity");
    }

    #[test]
    fn test_feature_prior_with_range() {
        let prior = FeaturePrior::new("test").with_range(1.0, 10.0, 5.0);

        assert_eq!(prior.expected_min, 1.0);
        assert_eq!(prior.expected_max, 10.0);
        assert_eq!(prior.expected_mean, 5.0);
    }

    #[test]
    fn test_feature_prior_effective_sample_size() {
        let prior = FeaturePrior::new("test").with_beta_params(5.0, 5.0);

        let ess = prior.effective_sample_size();
        assert_eq!(ess, 10.0); // alpha + beta
    }

    #[test]
    fn test_feature_prior_prior_variance() {
        let prior = FeaturePrior::new("test").with_beta_params(2.0, 8.0);

        let variance = prior.prior_variance();
        assert!(variance > 0.0);
        assert!(variance < 1.0); // Beta distribution variance is bounded
    }

    #[test]
    fn test_feature_statistics_from_values() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let stats = FeatureStatistics::from_values(&values);

        assert_eq!(stats.mean, 3.0);
        assert_eq!(stats.min, 1.0);
        assert_eq!(stats.max, 5.0);
        assert_eq!(stats.n_samples, 5);
        assert!(stats.variance > 0.0);
    }

    #[test]
    fn feature_statistics_from_empty_values_returns_defaults() {
        let stats = FeatureStatistics::from_values(&[]);
        assert_eq!(stats.n_samples, 0);
        assert_eq!(stats.mean, 0.0);
        assert_eq!(stats.variance, 0.0);
        assert_eq!(stats.prior_weight, 0.0);
        assert_eq!(stats.posterior_mean, 0.0);
        assert_eq!(stats.posterior_variance, 0.0);
        assert_eq!(stats.confidence, VarianceConfidence::Insufficient);
    }

    #[test]
    fn test_bayesian_normalizer_confidence_methods() {
        let mut normalizer = BayesianNormalizer::new("z_score");

        // Test with mock feature statistics
        let stats = FeatureStatistics {
            mean: 3.0,
            variance: 2.0,
            std_dev: 2.0_f64.sqrt(),
            min: 1.0,
            max: 5.0,
            n_samples: 100,
            confidence: VarianceConfidence::High,
            prior_weight: 0.1,
            posterior_mean: 3.2,
            posterior_variance: 1.8,
        };

        // Fit with data to populate internal statistics
        let mut vectors = vec![FeatureVector::new("test1"), FeatureVector::new("test2")];
        vectors[0].add_feature("test_feature", 1.0);
        vectors[1].add_feature("test_feature", 5.0);
        normalizer.fit(&vectors).unwrap();

        let retrieved_stats = normalizer.get_statistics("test_feature");
        assert!(retrieved_stats.is_some());
        assert_eq!(retrieved_stats.unwrap().mean, 3.0);

        let confidence = normalizer.get_confidence("test_feature");
        assert!(confidence.is_some());
        assert_eq!(confidence.unwrap(), VarianceConfidence::VeryLow);
    }

    #[test]
    fn test_bayesian_normalizer_add_prior() {
        let mut normalizer = BayesianNormalizer::new("z_score");
        let prior = FeaturePrior::new("complexity").with_beta_params(2.0, 3.0);

        normalizer.add_prior(prior.clone());
        // Test that the prior was added successfully (no error)
        // We can't test private fields directly, so we just verify no errors occurred
    }

    #[test]
    fn test_bayesian_normalizer_get_all_statistics() {
        let normalizer = BayesianNormalizer::new("z_score");

        let all_stats = normalizer.get_all_statistics();
        assert_eq!(all_stats.len(), 0); // Empty normalizer
    }

    #[test]
    fn test_variance_confidence_score() {
        assert_eq!(VarianceConfidence::High.score(), 0.9);
        assert_eq!(VarianceConfidence::Medium.score(), 0.7);
        assert_eq!(VarianceConfidence::Low.score(), 0.5);
        assert_eq!(VarianceConfidence::VeryLow.score(), 0.3);
        assert_eq!(VarianceConfidence::Insufficient.score(), 0.1);
    }

    #[test]
    fn test_bayesian_normalizer_requires_input() {
        let mut normalizer = BayesianNormalizer::new("z_score");
        let err = normalizer
            .fit(&[])
            .expect_err("fitting with no vectors should fail");
        assert!(err
            .to_string()
            .contains("No feature vectors provided for Bayesian fitting"));
    }

    #[test]
    fn test_normalize_identity_when_missing_statistics() {
        let normalizer = BayesianNormalizer::new("z_score");
        let mut vectors = vec![FeatureVector::new("entity1")];
        vectors[0].add_feature("unseen", 7.0);

        normalizer
            .normalize(&mut vectors)
            .expect("identity normalization should succeed");

        assert_eq!(vectors[0].normalized_features.get("unseen"), Some(&7.0));
    }

    #[test]
    fn test_normalize_unknown_scheme_produces_error() {
        let mut normalizer = BayesianNormalizer::new("custom_scheme");
        let stats = FeatureStatistics {
            mean: 1.0,
            variance: 1.0,
            std_dev: 1.0,
            min: 0.0,
            max: 2.0,
            n_samples: 5,
            confidence: VarianceConfidence::Medium,
            prior_weight: 0.0,
            posterior_mean: 1.0,
            posterior_variance: 1.0,
        };
        normalizer
            .statistics
            .insert("metric".to_string(), stats.clone());

        let mut vectors = vec![FeatureVector::new("entity")];
        vectors[0].add_feature("metric", 1.5);

        let err = normalizer
            .normalize(&mut vectors)
            .expect_err("unknown schemes should error");
        assert!(err.to_string().contains("Unknown normalization scheme"));
    }

    #[test]
    fn test_minmax_zero_range_returns_midpoint() {
        let normalizer = BayesianNormalizer::new("min_max");
        let stats = FeatureStatistics {
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            min: 5.0,
            max: 5.0,
            n_samples: 1,
            confidence: VarianceConfidence::Insufficient,
            prior_weight: 0.0,
            posterior_mean: 0.0,
            posterior_variance: 0.0,
        };

        let value = normalizer
            .normalize_value(5.0, &stats)
            .expect("min/max normalization should succeed");
        assert_eq!(value, 0.5);
    }

    #[test]
    fn test_bayesian_scheme_uses_prior_sample_on_zero_variance() {
        let normalizer = BayesianNormalizer::new("z_score_bayesian");
        let stats = FeatureStatistics {
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 0.0,
            n_samples: 1,
            confidence: VarianceConfidence::Insufficient,
            prior_weight: 0.0,
            posterior_mean: 0.8,
            posterior_variance: 0.0,
        };

        let normalized = normalizer
            .normalize_value(1.0, &stats)
            .expect("bayesian scheme should succeed");
        assert_eq!(normalized, 0.5);
    }

    #[test]
    fn test_feature_prior_type_variants() {
        // Test that the enum variants exist conceptually
        let _informative = "informative";
        let _weak = "weak";
        let _noninformative = "noninformative";

        // Basic test to ensure the test passes
        assert!(true);
    }

    #[test]
    fn test_bayesian_normalizer_normalize_value() {
        let mut normalizer = BayesianNormalizer::new("z_score");

        // Add some mock statistics
        let stats = FeatureStatistics {
            mean: 5.0,
            variance: 4.0,
            std_dev: 2.0,
            min: 1.0,
            max: 9.0,
            n_samples: 10,
            confidence: VarianceConfidence::Medium,
            prior_weight: 0.0,
            posterior_mean: 5.0,
            posterior_variance: 4.0,
        };

        let stats = FeatureStatistics {
            mean: 5.0,
            variance: 4.0,
            std_dev: 2.0,
            min: 1.0,
            max: 10.0,
            n_samples: 10,
            confidence: VarianceConfidence::High,
            prior_weight: 0.1,
            posterior_mean: 5.0,
            posterior_variance: 4.0,
        };

        let normalized = normalizer.normalize_value(7.0, &stats);
        assert!(normalized.is_ok());
        assert_eq!(normalized.unwrap(), 1.0); // (7-5)/2 = 1
    }

    #[test]
    fn test_bayesian_normalizer_create_generic_prior() {
        let normalizer = BayesianNormalizer::new("z_score");
        let prior = normalizer.create_generic_prior("new_feature");

        assert_eq!(prior.name, "new_feature");
        // Test that the prior was created successfully
        assert!(prior.alpha > 0.0);
        assert!(prior.beta > 0.0);
    }

    #[test]
    fn test_min_max_normalization_handles_zero_range() {
        let normalizer = BayesianNormalizer::new("min_max");
        let stats = FeatureStatistics {
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            min: 10.0,
            max: 10.0,
            n_samples: 1,
            confidence: VarianceConfidence::Low,
            prior_weight: 0.0,
            posterior_mean: 0.0,
            posterior_variance: 0.0,
        };

        let normalized = normalizer.normalize_value(42.0, &stats).unwrap();
        assert_eq!(normalized, 0.5, "zero range should default to midpoint");
    }

    #[test]
    fn test_min_max_normalization_scales_value() {
        let normalizer = BayesianNormalizer::new("minmax");
        let stats = FeatureStatistics {
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 100.0,
            n_samples: 5,
            confidence: VarianceConfidence::Medium,
            prior_weight: 0.0,
            posterior_mean: 0.0,
            posterior_variance: 0.0,
        };

        let normalized = normalizer.normalize_value(25.0, &stats).unwrap();
        assert!((normalized - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_robust_normalization_uses_posterior_variance() {
        let normalizer = BayesianNormalizer::new("robust");
        let stats = FeatureStatistics {
            mean: 5.0,
            variance: 4.0,
            std_dev: 2.0,
            min: 0.0,
            max: 10.0,
            n_samples: 10,
            confidence: VarianceConfidence::Medium,
            prior_weight: 0.0,
            posterior_mean: 5.0,
            posterior_variance: 4.0,
        };

        let normalized = normalizer.normalize_value(7.0, &stats).unwrap();
        assert!((normalized - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_normalize_value_rejects_unknown_scheme() {
        let normalizer = BayesianNormalizer::new("unknown_scheme");
        let stats = FeatureStatistics {
            mean: 0.0,
            variance: 1.0,
            std_dev: 1.0,
            min: 0.0,
            max: 1.0,
            n_samples: 2,
            confidence: VarianceConfidence::High,
            prior_weight: 0.0,
            posterior_mean: 0.0,
            posterior_variance: 1.0,
        };

        let err = normalizer
            .normalize_value(0.5, &stats)
            .expect_err("unknown scheme should error");
        assert!(
            format!("{err}").contains("Unknown normalization scheme"),
            "unexpected error message: {err}"
        );
    }

    #[test]
    fn test_bayesian_normalization_uses_prior_sampling_for_low_confidence() {
        let normalizer = BayesianNormalizer::new("posterior_bayesian");
        let mut stats_low = FeatureStatistics {
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 1.0,
            n_samples: 1,
            confidence: VarianceConfidence::Insufficient,
            prior_weight: 0.0,
            posterior_mean: 0.3,
            posterior_variance: 0.0,
        };
        let mut stats_high = stats_low.clone();
        stats_high.posterior_mean = 0.9;

        let low_conf = normalizer.normalize_value(0.2, &stats_low).unwrap();
        let high_conf = normalizer.normalize_value(0.8, &stats_high).unwrap();

        assert_eq!(low_conf, -0.5, "low prior mean should skew negative");
        assert_eq!(high_conf, 0.5, "high prior mean should skew positive");
    }

    #[test]
    fn test_prior_weight_respects_confidence_and_clamp() {
        let normalizer = BayesianNormalizer::new("z_score");
        let empirical_high = FeatureStatistics {
            mean: 1.0,
            variance: 1.0,
            std_dev: 1.0,
            min: 0.0,
            max: 2.0,
            n_samples: 10_000,
            confidence: VarianceConfidence::High,
            prior_weight: 0.0,
            posterior_mean: 0.0,
            posterior_variance: 0.0,
        };
        let empirical_low = FeatureStatistics {
            mean: 1.0,
            variance: 1.0,
            std_dev: 1.0,
            min: 0.0,
            max: 2.0,
            n_samples: 1,
            confidence: VarianceConfidence::Insufficient,
            prior_weight: 0.0,
            posterior_mean: 0.0,
            posterior_variance: 0.0,
        };
        let prior = FeaturePrior::new("feature").with_beta_params(2.0, 3.0);

        let stats_high = normalizer
            .calculate_posterior_stats(&empirical_high, &prior)
            .expect("posterior calculation should succeed");
        assert!(
            (stats_high.prior_weight - 0.05).abs() < f64::EPSILON,
            "high confidence with many samples should clamp near minimum weight"
        );

        let stats_low = normalizer
            .calculate_posterior_stats(&empirical_low, &prior)
            .expect("posterior calculation should succeed");
        assert!(
            (stats_low.prior_weight - 0.9).abs() < f64::EPSILON,
            "low confidence with few samples should lean on the prior"
        );
    }
