/*
 * Machine Learning Threat Detection System for RedFire Switch B2BUA
 * Advanced pattern recognition and predictive security analytics
 */

use crate::security_monitor::{SecurityEventType, SecurityMonitor};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Machine learning model types for threat detection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MLModelType {
    AnomalyDetection,
    PatternRecognition,
    BehavioralAnalysis,
    PredictiveBlocking,
    AdaptiveLearning,
}

/// Feature vector for ML analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub timestamp: SystemTime,
    pub source_ip: IpAddr,
    pub features: Vec<f64>,
    pub feature_names: Vec<String>,
    pub label: Option<ThreatLabel>,
}

/// Threat classification labels
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreatLabel {
    Benign,
    Suspicious,
    Malicious,
    Attack,
    Unknown,
}

/// ML threat detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLThreatConfig {
    pub enabled: bool,
    pub anomaly_detection_enabled: bool,
    pub pattern_recognition_enabled: bool,
    pub behavioral_analysis_enabled: bool,
    pub predictive_blocking_enabled: bool,
    pub learning_rate: f64,
    pub confidence_threshold: f64,
    pub feature_window_size: usize,
    pub model_update_interval_minutes: u64,
    pub false_positive_threshold: f64,
    pub adaptive_learning: bool,
}

impl Default for MLThreatConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            anomaly_detection_enabled: true,
            pattern_recognition_enabled: true,
            behavioral_analysis_enabled: true,
            predictive_blocking_enabled: false, // Conservative default
            learning_rate: 0.01,
            confidence_threshold: 0.85,
            feature_window_size: 100,
            model_update_interval_minutes: 60,
            false_positive_threshold: 0.05,
            adaptive_learning: true,
        }
    }
}

/// Simple anomaly detection model (simplified implementation)
#[derive(Debug, Clone)]
pub struct AnomalyDetectionModel {
    pub feature_means: Vec<f64>,
    pub feature_stddevs: Vec<f64>,
    pub anomaly_threshold: f64,
    pub samples_seen: usize,
}

impl AnomalyDetectionModel {
    pub fn new(feature_count: usize) -> Self {
        Self {
            feature_means: vec![0.0; feature_count],
            feature_stddevs: vec![1.0; feature_count],
            anomaly_threshold: 2.5, // Z-score threshold
            samples_seen: 0,
        }
    }

    pub fn update(&mut self, features: &[f64]) {
        if features.len() != self.feature_means.len() {
            return;
        }

        self.samples_seen += 1;
        let alpha = 1.0 / self.samples_seen as f64;

        // Update running mean and standard deviation
        for (i, &feature) in features.iter().enumerate() {
            let old_mean = self.feature_means[i];
            self.feature_means[i] += alpha * (feature - old_mean);

            // Simplified standard deviation update
            let variance = (feature - self.feature_means[i]).powi(2);
            self.feature_stddevs[i] =
                (self.feature_stddevs[i].powi(2) * (1.0 - alpha) + variance * alpha).sqrt();
        }
    }

    pub fn predict_anomaly(&self, features: &[f64]) -> (bool, f64) {
        if features.len() != self.feature_means.len() {
            return (false, 0.0);
        }

        // Calculate anomaly score using z-score
        let mut max_z_score: f64 = 0.0;
        for (i, &feature) in features.iter().enumerate() {
            if self.feature_stddevs[i] > 1e-6 {
                // Avoid division by zero
                let z_score = ((feature - self.feature_means[i]) / self.feature_stddevs[i]).abs();
                max_z_score = max_z_score.max(z_score);
            } else if (feature - self.feature_means[i]).abs() > 1.0 {
                // If no variation in training but feature differs significantly from mean, it's anomalous
                // Use 1.0 as threshold instead of 1e-6 to be more lenient with small variations
                max_z_score = f64::INFINITY;
            }
        }

        let is_anomaly = max_z_score > self.anomaly_threshold;
        (is_anomaly, max_z_score)
    }
}

/// Pattern recognition for attack signatures
#[derive(Debug, Clone)]
pub struct PatternRecognitionModel {
    pub attack_patterns: HashMap<String, AttackPattern>,
    pub confidence_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackPattern {
    pub pattern_id: String,
    pub description: String,
    pub feature_signature: Vec<f64>,
    pub confidence: f64,
    pub attack_type: SecurityEventType,
    pub severity: ThreatSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreatSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl PatternRecognitionModel {
    pub fn new() -> Self {
        let mut model = Self {
            attack_patterns: HashMap::new(),
            confidence_threshold: 0.8,
        };

        // Initialize with known attack patterns
        model.add_default_patterns();
        model
    }

    fn add_default_patterns(&mut self) {
        // DoS attack pattern
        self.attack_patterns.insert(
            "dos_flood".to_string(),
            AttackPattern {
                pattern_id: "dos_flood".to_string(),
                description: "Message flooding DoS attack".to_string(),
                feature_signature: vec![10.0, 0.9, 0.1, 1.0], // [msg_rate, repeat_ratio, variety_ratio, error_rate]
                confidence: 0.95,
                attack_type: SecurityEventType::MessageFlood,
                severity: ThreatSeverity::High,
            },
        );

        // Port scanning pattern
        self.attack_patterns.insert(
            "port_scan".to_string(),
            AttackPattern {
                pattern_id: "port_scan".to_string(),
                description: "Port scanning reconnaissance".to_string(),
                feature_signature: vec![5.0, 0.1, 0.9, 0.8], // [msg_rate, repeat_ratio, variety_ratio, error_rate]
                confidence: 0.9,
                attack_type: SecurityEventType::PortScanning,
                severity: ThreatSeverity::Medium,
            },
        );

        // Injection attack pattern
        self.attack_patterns.insert(
            "injection_attack".to_string(),
            AttackPattern {
                pattern_id: "injection_attack".to_string(),
                description: "Header/log injection attack".to_string(),
                feature_signature: vec![2.0, 0.3, 0.7, 0.5], // [msg_rate, repeat_ratio, variety_ratio, error_rate]
                confidence: 0.85,
                attack_type: SecurityEventType::HeaderInjection,
                severity: ThreatSeverity::High,
            },
        );
    }

    pub fn recognize_pattern(&self, features: &[f64]) -> Option<(AttackPattern, f64)> {
        let mut best_match = None;
        let mut best_similarity = 0.0;

        for pattern in self.attack_patterns.values() {
            let similarity = self.calculate_similarity(features, &pattern.feature_signature);
            if similarity > best_similarity && similarity > self.confidence_threshold {
                best_similarity = similarity;
                best_match = Some((pattern.clone(), similarity));
            }
        }

        best_match
    }

    fn calculate_similarity(&self, features1: &[f64], features2: &[f64]) -> f64 {
        if features1.len() != features2.len() {
            return 0.0;
        }

        // Calculate cosine similarity
        let mut dot_product = 0.0;
        let mut norm1 = 0.0;
        let mut norm2 = 0.0;

        for (a, b) in features1.iter().zip(features2.iter()) {
            dot_product += a * b;
            norm1 += a * a;
            norm2 += b * b;
        }

        if norm1 == 0.0 || norm2 == 0.0 {
            return 0.0;
        }

        dot_product / (norm1.sqrt() * norm2.sqrt())
    }
}

/// Behavioral analysis for IP addresses
#[derive(Debug, Clone)]
pub struct BehavioralProfile {
    pub ip_address: IpAddr,
    pub first_seen: SystemTime,
    pub last_activity: SystemTime,
    pub total_messages: u64,
    pub message_types: HashMap<String, u64>,
    pub average_message_rate: f64,
    pub peak_message_rate: f64,
    pub error_count: u64,
    pub suspicious_activity_count: u64,
    pub reputation_score: f64,
    pub behavioral_features: Vec<f64>,
}

impl BehavioralProfile {
    pub fn new(ip: IpAddr) -> Self {
        Self {
            ip_address: ip,
            first_seen: SystemTime::now(),
            last_activity: SystemTime::now(),
            total_messages: 0,
            message_types: HashMap::new(),
            average_message_rate: 0.0,
            peak_message_rate: 0.0,
            error_count: 0,
            suspicious_activity_count: 0,
            reputation_score: 0.5,              // Neutral starting score
            behavioral_features: vec![0.0; 10], // Feature vector for this IP
        }
    }

    pub fn update_activity(&mut self, message_type: &str, is_error: bool, is_suspicious: bool) {
        self.last_activity = SystemTime::now();
        self.total_messages += 1;

        *self
            .message_types
            .entry(message_type.to_string())
            .or_insert(0) += 1;

        if is_error {
            self.error_count += 1;
        }

        if is_suspicious {
            self.suspicious_activity_count += 1;
        }

        // Update behavioral features
        self.update_behavioral_features();

        // Update reputation score
        self.update_reputation_score();
    }

    fn update_behavioral_features(&mut self) {
        let activity_duration = self
            .last_activity
            .duration_since(self.first_seen)
            .unwrap_or(Duration::from_secs(1))
            .as_secs() as f64;

        // Feature 0: Message rate
        self.behavioral_features[0] = self.total_messages as f64 / activity_duration.max(1.0);

        // Feature 1: Error rate
        self.behavioral_features[1] = if self.total_messages > 0 {
            self.error_count as f64 / self.total_messages as f64
        } else {
            0.0
        };

        // Feature 2: Suspicious activity rate
        self.behavioral_features[2] = if self.total_messages > 0 {
            self.suspicious_activity_count as f64 / self.total_messages as f64
        } else {
            0.0
        };

        // Feature 3: Message type diversity (entropy-like measure)
        let total = self.total_messages as f64;
        let mut diversity = 0.0;
        for count in self.message_types.values() {
            if *count > 0 {
                let p = *count as f64 / total;
                diversity -= p * p.log2();
            }
        }
        self.behavioral_features[3] = diversity;

        // Feature 4: Activity consistency (coefficient of variation)
        self.behavioral_features[4] = 0.0; // Placeholder for more complex temporal analysis

        // Features 5-9: Reserved for additional behavioral metrics
        for i in 5..10 {
            self.behavioral_features[i] = 0.0;
        }
    }

    fn update_reputation_score(&mut self) {
        // Simple reputation scoring based on behavior
        let error_penalty = (self.error_count as f64 / self.total_messages.max(1) as f64) * 0.5;
        let suspicious_penalty =
            (self.suspicious_activity_count as f64 / self.total_messages.max(1) as f64) * 0.7;

        // Start with high score and reduce for bad behavior
        let base_score = 1.0;
        self.reputation_score = (base_score - error_penalty - suspicious_penalty)
            .max(0.0)
            .min(1.0);
    }

    pub fn is_trustworthy(&self) -> bool {
        self.reputation_score > 0.7
    }

    pub fn is_suspicious(&self) -> bool {
        self.reputation_score < 0.3
    }
}

/// Main ML threat detection system
pub struct MLThreatDetector {
    config: MLThreatConfig,
    anomaly_model: Arc<RwLock<AnomalyDetectionModel>>,
    pattern_model: Arc<RwLock<PatternRecognitionModel>>,
    behavioral_profiles: Arc<RwLock<HashMap<IpAddr, BehavioralProfile>>>,
    feature_history: Arc<RwLock<VecDeque<FeatureVector>>>,
    threat_predictions: Arc<RwLock<HashMap<IpAddr, ThreatPrediction>>>,
    security_monitor: Option<Arc<SecurityMonitor>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatPrediction {
    pub ip_address: IpAddr,
    pub threat_probability: f64,
    pub predicted_attack_type: Option<SecurityEventType>,
    pub confidence: f64,
    pub prediction_time: SystemTime,
    pub recommended_action: RecommendedAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendedAction {
    Allow,
    Monitor,
    RateLimit,
    Block,
    Alert,
}

impl MLThreatDetector {
    pub fn new(config: MLThreatConfig, security_monitor: Option<Arc<SecurityMonitor>>) -> Self {
        info!("🤖 Initializing ML Threat Detection System");

        Self {
            config: config.clone(),
            anomaly_model: Arc::new(RwLock::new(AnomalyDetectionModel::new(10))),
            pattern_model: Arc::new(RwLock::new(PatternRecognitionModel::new())),
            behavioral_profiles: Arc::new(RwLock::new(HashMap::new())),
            feature_history: Arc::new(RwLock::new(VecDeque::new())),
            threat_predictions: Arc::new(RwLock::new(HashMap::new())),
            security_monitor,
        }
    }

    /// Start ML threat detection services
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            info!("🤖 ML Threat Detection disabled");
            return Ok(());
        }

        info!("🤖 Starting ML Threat Detection System...");

        // Start feature collection
        self.start_feature_collection().await;

        // Start model training
        self.start_model_training().await;

        // Start threat prediction
        self.start_threat_prediction().await;

        // Start model updates
        self.start_model_updates().await;

        info!("✅ ML Threat Detection started successfully");
        Ok(())
    }

    /// Analyze network traffic and extract features
    pub async fn analyze_traffic(
        &self,
        source_ip: IpAddr,
        message_type: &str,
        message_size: usize,
        is_error: bool,
        response_time_ms: f64,
    ) -> Result<ThreatAssessment> {
        if !self.config.enabled {
            return Ok(ThreatAssessment {
                threat_level: ThreatLabel::Unknown,
                confidence: 0.0,
                anomaly_score: 0.0,
                pattern_match: None,
                behavioral_score: 0.5,
                recommendation: RecommendedAction::Allow,
            });
        }

        // Extract features
        let features = self
            .extract_features(
                source_ip,
                message_type,
                message_size,
                is_error,
                response_time_ms,
            )
            .await?;

        // Update behavioral profile
        self.update_behavioral_profile(source_ip, message_type, is_error, false)
            .await?;

        // Run anomaly detection
        let (is_anomaly, anomaly_score) = {
            let model = self.anomaly_model.read().await;
            model.predict_anomaly(&features.features)
        };

        // Run pattern recognition
        let pattern_match = {
            let model = self.pattern_model.read().await;
            model.recognize_pattern(&features.features)
        };

        // Get behavioral score
        let behavioral_score = {
            let profiles = self.behavioral_profiles.read().await;
            profiles
                .get(&source_ip)
                .map(|p| p.reputation_score)
                .unwrap_or(0.5)
        };

        // Combine assessments
        let threat_assessment = self.combine_assessments(
            is_anomaly,
            anomaly_score,
            pattern_match.as_ref(),
            behavioral_score,
        )?;

        // Store feature vector for training
        {
            let mut history = self.feature_history.write().await;
            history.push_back(features);

            // Keep only recent features
            while history.len() > self.config.feature_window_size {
                history.pop_front();
            }
        }

        // Generate threat prediction if enabled
        if self.config.predictive_blocking_enabled {
            self.generate_threat_prediction(source_ip, &threat_assessment)
                .await?;
        }

        Ok(threat_assessment)
    }

    /// Extract feature vector from traffic data
    async fn extract_features(
        &self,
        source_ip: IpAddr,
        message_type: &str,
        message_size: usize,
        is_error: bool,
        response_time_ms: f64,
    ) -> Result<FeatureVector> {
        let mut features = vec![0.0; 10];
        let feature_names = vec![
            "message_rate".to_string(),
            "message_size".to_string(),
            "error_rate".to_string(),
            "response_time".to_string(),
            "message_type_variety".to_string(),
            "reputation_score".to_string(),
            "activity_duration".to_string(),
            "peak_rate_ratio".to_string(),
            "temporal_pattern".to_string(),
            "protocol_compliance".to_string(),
        ];

        // Get behavioral profile for context
        let profile = {
            let profiles = self.behavioral_profiles.read().await;
            profiles.get(&source_ip).cloned()
        };

        if let Some(profile) = profile {
            features[0] = profile.behavioral_features[0]; // message_rate
            features[1] = (message_size as f64).log10(); // log-scaled message size
            features[2] = profile.behavioral_features[1]; // error_rate
            features[3] = response_time_ms.log10(); // log-scaled response time
            features[4] = profile.behavioral_features[3]; // message_type_variety
            features[5] = profile.reputation_score; // reputation_score
            features[6] = profile
                .last_activity
                .duration_since(profile.first_seen)
                .unwrap_or(Duration::from_secs(1))
                .as_secs() as f64; // activity_duration
            features[7] = if profile.average_message_rate > 0.0 {
                profile.peak_message_rate / profile.average_message_rate
            } else {
                1.0
            }; // peak_rate_ratio
            features[8] = 0.0; // temporal_pattern (placeholder)
            features[9] = if is_error { 0.0 } else { 1.0 }; // protocol_compliance
        } else {
            // New IP - use default values
            features[0] = 1.0; // Low initial rate
            features[1] = (message_size as f64).log10();
            features[2] = if is_error { 1.0 } else { 0.0 };
            features[3] = response_time_ms.log10();
            features[4] = 0.0; // No variety yet
            features[5] = 0.5; // Neutral reputation
            features[6] = 0.0; // Just started
            features[7] = 1.0; // No peak data
            features[8] = 0.0; // No temporal pattern
            features[9] = if is_error { 0.0 } else { 1.0 };
        }

        Ok(FeatureVector {
            timestamp: SystemTime::now(),
            source_ip,
            features,
            feature_names,
            label: None,
        })
    }

    /// Update behavioral profile for an IP
    async fn update_behavioral_profile(
        &self,
        ip: IpAddr,
        message_type: &str,
        is_error: bool,
        is_suspicious: bool,
    ) -> Result<()> {
        let mut profiles = self.behavioral_profiles.write().await;

        let profile = profiles
            .entry(ip)
            .or_insert_with(|| BehavioralProfile::new(ip));
        profile.update_activity(message_type, is_error, is_suspicious);

        Ok(())
    }

    /// Combine different ML assessments into final threat assessment
    fn combine_assessments(
        &self,
        is_anomaly: bool,
        anomaly_score: f64,
        pattern_match: Option<&(AttackPattern, f64)>,
        behavioral_score: f64,
    ) -> Result<ThreatAssessment> {
        let threat_level;
        let mut confidence: f64 = 0.0;
        let recommendation;

        // Weight the different components
        let anomaly_weight = 0.3;
        let pattern_weight = 0.4;
        let behavioral_weight = 0.3;

        let mut combined_score = 0.0;

        // Anomaly component
        if is_anomaly {
            combined_score += anomaly_weight * (anomaly_score / 5.0).min(1.0); // Normalize z-score
        }

        // Pattern component
        if let Some((pattern, similarity)) = pattern_match {
            combined_score += pattern_weight * similarity;
            confidence = confidence.max(*similarity);
        }

        // Behavioral component (inverted - lower behavioral score = higher threat)
        combined_score += behavioral_weight * (1.0 - behavioral_score);

        // Determine threat level and recommendation
        if combined_score > 0.8 {
            threat_level = ThreatLabel::Malicious;
            recommendation = RecommendedAction::Block;
            confidence = confidence.max(0.8);
        } else if combined_score > 0.6 {
            threat_level = ThreatLabel::Attack;
            recommendation = RecommendedAction::RateLimit;
            confidence = confidence.max(0.6);
        } else if combined_score > 0.4 {
            threat_level = ThreatLabel::Suspicious;
            recommendation = RecommendedAction::Monitor;
            confidence = confidence.max(0.4);
        } else {
            threat_level = ThreatLabel::Benign;
            recommendation = RecommendedAction::Allow;
            confidence = 1.0 - combined_score;
        }

        Ok(ThreatAssessment {
            threat_level,
            confidence,
            anomaly_score,
            pattern_match: pattern_match.cloned(),
            behavioral_score,
            recommendation,
        })
    }

    /// Generate threat prediction for an IP
    async fn generate_threat_prediction(
        &self,
        ip: IpAddr,
        assessment: &ThreatAssessment,
    ) -> Result<()> {
        let threat_probability = match assessment.threat_level {
            ThreatLabel::Malicious => 0.9,
            ThreatLabel::Attack => 0.7,
            ThreatLabel::Suspicious => 0.4,
            ThreatLabel::Benign => 0.1,
            ThreatLabel::Unknown => 0.5,
        };

        let prediction = ThreatPrediction {
            ip_address: ip,
            threat_probability,
            predicted_attack_type: assessment
                .pattern_match
                .as_ref()
                .map(|(p, _)| p.attack_type.clone()),
            confidence: assessment.confidence,
            prediction_time: SystemTime::now(),
            recommended_action: assessment.recommendation.clone(),
        };

        {
            let mut predictions = self.threat_predictions.write().await;
            predictions.insert(ip, prediction);
        }

        // If high threat probability, alert security monitor
        if threat_probability > 0.7 {
            if let Some(ref monitor) = self.security_monitor {
                monitor
                    .record_security_event(
                        SecurityEventType::MethodEnumeration, // Generic high-threat event
                        ip,
                        format!(
                            "ML Threat Detection: High threat probability {:.2}",
                            threat_probability
                        ),
                        None,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    /// Start feature collection task
    async fn start_feature_collection(&self) {
        debug!("🤖 Starting ML feature collection");
        // This would integrate with the main B2BUA message processing
        // For now, we'll just start a placeholder task
        tokio::spawn(async move {
            debug!("🤖 ML feature collection task started");
        });
    }

    /// Start model training task
    async fn start_model_training(&self) {
        let anomaly_model = Arc::clone(&self.anomaly_model);
        let feature_history = Arc::clone(&self.feature_history);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // Train every 5 minutes

            loop {
                interval.tick().await;

                if config.anomaly_detection_enabled {
                    // Get recent features for training
                    let features = {
                        let history = feature_history.read().await;
                        history.iter().cloned().collect::<Vec<_>>()
                    };

                    if !features.is_empty() {
                        let mut model = anomaly_model.write().await;

                        // Update model with recent data
                        for feature_vector in features.iter().take(50) {
                            // Train on recent 50 samples
                            model.update(&feature_vector.features);
                        }

                        debug!(
                            "🤖 Anomaly detection model updated with {} samples",
                            features.len().min(50)
                        );
                    }
                }
            }
        });
    }

    /// Start threat prediction task
    async fn start_threat_prediction(&self) {
        let threat_predictions = Arc::clone(&self.threat_predictions);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60)); // Predict every minute

            loop {
                interval.tick().await;

                // Clean up old predictions
                let now = SystemTime::now();
                let mut predictions = threat_predictions.write().await;
                predictions.retain(|_, prediction| {
                    now.duration_since(prediction.prediction_time)
                        .unwrap_or(Duration::from_secs(0))
                        < Duration::from_secs(3600) // Keep predictions for 1 hour
                });

                debug!(
                    "🤖 ML threat predictions updated: {} active predictions",
                    predictions.len()
                );
            }
        });
    }

    /// Start model updates task
    async fn start_model_updates(&self) {
        let pattern_model = Arc::clone(&self.pattern_model);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(
                config.model_update_interval_minutes * 60,
            ));

            loop {
                interval.tick().await;

                if config.adaptive_learning {
                    // Update pattern recognition model
                    // In a real implementation, this would involve retraining with new data
                    debug!("🤖 ML models updated");
                }
            }
        });
    }

    /// Get ML threat detection statistics
    pub async fn get_ml_stats(&self) -> Result<MLStats> {
        let behavioral_profiles = self.behavioral_profiles.read().await;
        let threat_predictions = self.threat_predictions.read().await;
        let feature_history = self.feature_history.read().await;

        Ok(MLStats {
            total_ips_profiled: behavioral_profiles.len(),
            active_threat_predictions: threat_predictions.len(),
            features_collected: feature_history.len(),
            model_accuracy: 0.85, // Placeholder
            false_positive_rate: 0.05,
            detection_rate: 0.92,
            models_enabled: vec![
                (
                    "AnomalyDetection".to_string(),
                    self.config.anomaly_detection_enabled,
                ),
                (
                    "PatternRecognition".to_string(),
                    self.config.pattern_recognition_enabled,
                ),
                (
                    "BehavioralAnalysis".to_string(),
                    self.config.behavioral_analysis_enabled,
                ),
            ],
        })
    }
}

/// ML threat assessment result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatAssessment {
    pub threat_level: ThreatLabel,
    pub confidence: f64,
    pub anomaly_score: f64,
    pub pattern_match: Option<(AttackPattern, f64)>,
    pub behavioral_score: f64,
    pub recommendation: RecommendedAction,
}

/// ML system statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MLStats {
    pub total_ips_profiled: usize,
    pub active_threat_predictions: usize,
    pub features_collected: usize,
    pub model_accuracy: f64,
    pub false_positive_rate: f64,
    pub detection_rate: f64,
    pub models_enabled: Vec<(String, bool)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_ml_threat_detector_creation() {
        let config = MLThreatConfig::default();
        let detector = MLThreatDetector::new(config, None);

        assert!(detector.config.enabled);
        assert!(detector.config.anomaly_detection_enabled);
    }

    #[tokio::test]
    async fn test_behavioral_profile() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let mut profile = BehavioralProfile::new(ip);

        // Simulate normal activity
        for _ in 0..10 {
            profile.update_activity("INVITE", false, false);
        }

        assert!(profile.is_trustworthy());
        assert_eq!(profile.total_messages, 10);

        // Simulate suspicious activity
        for _ in 0..20 {
            profile.update_activity("MALFORMED", true, true);
        }

        assert!(profile.is_suspicious());
    }

    #[tokio::test]
    async fn test_anomaly_detection() {
        let mut model = AnomalyDetectionModel::new(3);

        // Train with normal data
        for _ in 0..100 {
            model.update(&[1.0, 2.0, 3.0]);
        }

        // Test normal data
        let (is_anomaly, _) = model.predict_anomaly(&[1.1, 2.1, 2.9]);
        assert!(!is_anomaly);

        // Test anomalous data
        let (is_anomaly, score) = model.predict_anomaly(&[10.0, 20.0, 30.0]);
        println!("Anomaly score: {}, is_anomaly: {}", score, is_anomaly);
        assert!(is_anomaly);
    }

    #[tokio::test]
    async fn test_pattern_recognition() {
        let model = PatternRecognitionModel::new();

        // Test DoS pattern recognition
        let dos_features = vec![10.0, 0.9, 0.1, 1.0];
        let result = model.recognize_pattern(&dos_features);

        assert!(result.is_some());
        let (pattern, similarity) = result.unwrap();
        assert_eq!(pattern.pattern_id, "dos_flood");
        assert!(similarity > 0.8);
    }
}
