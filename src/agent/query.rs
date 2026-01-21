//! Query parsing and intent detection
//!
//! This module parses user questions and determines the type of query
//! (state check, prediction, calculation, comparison, etc.)

use serde::{Deserialize, Serialize};

/// Type of query the user is asking
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryType {
    /// Current system state ("What's my GPU temp?", "Show memory usage")
    State,

    /// Prediction query ("When will training complete?", "ETA?")
    Prediction,

    /// Energy/power calculation ("How much power?", "Cost estimate?")
    Energy,

    /// Comparison ("Is X faster than Y?", "Which GPU is better?")
    Comparison,

    /// Recommendation ("Should I upgrade?", "Optimize settings?")
    Recommendation,

    /// Mathematical calculation ("Average of last 10 readings?")
    Calculation,

    /// Historical trend ("Show GPU usage over time")
    Historical,

    /// General question
    General,
}

impl QueryType {
    /// Detect query type from question text
    fn detect(question: &str) -> Self {
        let q = question.to_lowercase();

        // Prediction indicators
        if q.contains("when will")
            || q.contains("eta")
            || q.contains("how long")
            || q.contains("time remaining")
            || q.contains("complete")
            || q.contains("finish")
        {
            return Self::Prediction;
        }

        // Energy indicators
        if q.contains("power")
            || q.contains("watt")
            || q.contains("energy")
            || q.contains("cost")
            || q.contains("electricity")
            || q.contains("kwh")
        {
            return Self::Energy;
        }

        // Comparison indicators
        if q.contains("compare")
            || q.contains("vs")
            || q.contains("versus")
            || q.contains("faster")
            || q.contains("slower")
            || q.contains("better")
            || q.contains("worse")
            || q.contains("difference")
        {
            return Self::Comparison;
        }

        // Recommendation indicators
        if q.contains("should i")
            || q.contains("recommend")
            || q.contains("suggest")
            || q.contains("optimize")
            || q.contains("improve")
            || q.contains("upgrade")
        {
            return Self::Recommendation;
        }

        // Calculation indicators
        if q.contains("average")
            || q.contains("mean")
            || q.contains("sum")
            || q.contains("total")
            || q.contains("calculate")
            || q.contains("compute")
        {
            return Self::Calculation;
        }

        // Historical indicators
        if q.contains("history")
            || q.contains("trend")
            || q.contains("over time")
            || q.contains("past")
            || q.contains("previous")
        {
            return Self::Historical;
        }

        // State indicators (default for monitoring queries)
        if q.contains("what")
            || q.contains("show")
            || q.contains("current")
            || q.contains("status")
            || q.contains("how much")
            || q.contains("usage")
            || q.contains("utilization")
            || q.contains("temp")
            || q.contains("memory")
            || q.contains("gpu")
            || q.contains("cpu")
        {
            return Self::State;
        }

        Self::General
    }
}

impl std::fmt::Display for QueryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::State => write!(f, "State"),
            Self::Prediction => write!(f, "Prediction"),
            Self::Energy => write!(f, "Energy"),
            Self::Comparison => write!(f, "Comparison"),
            Self::Recommendation => write!(f, "Recommendation"),
            Self::Calculation => write!(f, "Calculation"),
            Self::Historical => write!(f, "Historical"),
            Self::General => write!(f, "General"),
        }
    }
}

/// Parsed query with extracted entities
#[derive(Debug, Clone)]
pub struct Query {
    /// Original question text
    pub text: String,

    /// Detected query type
    pub query_type: QueryType,

    /// Extracted GPU indices (if query mentions specific GPUs)
    pub gpu_indices: Vec<usize>,

    /// Whether query is about all GPUs
    pub all_gpus: bool,

    /// Extracted numeric values (for thresholds, comparisons)
    pub numbers: Vec<f64>,

    /// Extracted time units (seconds, minutes, hours)
    pub time_unit: Option<String>,
}

impl Query {
    /// Parse question into structured query
    pub fn parse(question: &str) -> Self {
        let query_type = QueryType::detect(question);
        let text = question.to_string();
        let q = question.to_lowercase();

        // Extract GPU indices
        let mut gpu_indices = Vec::new();
        for (i, word) in q.split_whitespace().enumerate() {
            if word.starts_with("gpu") {
                // Try to parse number after "gpu"
                if let Some(num_str) = word.strip_prefix("gpu") {
                    if let Ok(idx) = num_str.parse::<usize>() {
                        gpu_indices.push(idx);
                    }
                }
            }
            // Check next word for "gpu 0", "gpu 1"
            if word == "gpu" {
                if let Some(next) = q.split_whitespace().nth(i + 1) {
                    if let Ok(idx) = next.parse::<usize>() {
                        gpu_indices.push(idx);
                    }
                }
            }
        }

        // Check for "all gpus" or "all"
        let all_gpus = q.contains("all gpu")
            || q.contains("all the gpu")
            || q.contains("every gpu")
            || (q.contains("all") && q.contains("gpu"));

        // Extract numbers
        let mut numbers = Vec::new();
        for word in q.split_whitespace() {
            // Try to parse as float
            if let Ok(num) = word
                .trim_matches(|c: char| !c.is_numeric() && c != '.')
                .parse::<f64>()
            {
                numbers.push(num);
            }
        }

        // Extract time units
        let time_unit = if q.contains("second") {
            Some("seconds".to_string())
        } else if q.contains("minute") {
            Some("minutes".to_string())
        } else if q.contains("hour") {
            Some("hours".to_string())
        } else if q.contains("day") {
            Some("days".to_string())
        } else {
            None
        };

        Self {
            text,
            query_type,
            gpu_indices,
            all_gpus,
            numbers,
            time_unit,
        }
    }

    /// Check if query mentions temperature
    pub fn mentions_temperature(&self) -> bool {
        let q = self.text.to_lowercase();
        q.contains("temp")
            || q.contains("temperature")
            || q.contains("hot")
            || q.contains("cool")
            || q.contains("thermal")
    }

    /// Check if query mentions memory
    pub fn mentions_memory(&self) -> bool {
        let q = self.text.to_lowercase();
        q.contains("memory") || q.contains("vram") || q.contains("ram") || q.contains("mem")
    }

    /// Check if query mentions utilization
    pub fn mentions_utilization(&self) -> bool {
        let q = self.text.to_lowercase();
        q.contains("util") || q.contains("usage") || q.contains("load") || q.contains("busy")
    }

    /// Check if query mentions power
    pub fn mentions_power(&self) -> bool {
        let q = self.text.to_lowercase();
        q.contains("power") || q.contains("watt") || q.contains("energy")
    }

    /// Check if query mentions processes
    pub fn mentions_processes(&self) -> bool {
        let q = self.text.to_lowercase();
        q.contains("process")
            || q.contains("program")
            || q.contains("application")
            || q.contains("task")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_type_detection() {
        assert_eq!(
            QueryType::detect("What's my GPU temperature?"),
            QueryType::State
        );
        assert_eq!(
            QueryType::detect("When will training complete?"),
            QueryType::Prediction
        );
        assert_eq!(
            QueryType::detect("How much power am I using?"),
            QueryType::Energy
        );
        assert_eq!(
            QueryType::detect("Compare GPU 0 vs GPU 1"),
            QueryType::Comparison
        );
        assert_eq!(
            QueryType::detect("Should I upgrade my RAM?"),
            QueryType::Recommendation
        );
        assert_eq!(
            QueryType::detect("Calculate average GPU usage"),
            QueryType::Calculation
        );
    }

    #[test]
    fn test_gpu_extraction() {
        let query = Query::parse("What's GPU 0 temperature?");
        assert_eq!(query.gpu_indices, vec![0]);

        let query = Query::parse("Compare GPU 1 and GPU 2");
        assert_eq!(query.gpu_indices, vec![1, 2]);

        let query = Query::parse("Show all GPUs");
        assert!(query.all_gpus);
    }

    #[test]
    fn test_number_extraction() {
        let query = Query::parse("Is GPU above 80 degrees?");
        assert!(query.numbers.contains(&80.0));

        let query = Query::parse("Usage over 50.5%");
        assert!(query.numbers.contains(&50.5));
    }

    #[test]
    fn test_mention_detection() {
        let query = Query::parse("What's my GPU temperature?");
        assert!(query.mentions_temperature());

        let query = Query::parse("Show memory usage");
        assert!(query.mentions_memory());

        let query = Query::parse("GPU utilization?");
        assert!(query.mentions_utilization());

        let query = Query::parse("Power consumption");
        assert!(query.mentions_power());
    }
}
