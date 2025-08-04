use crate::config::{LaunchConfig, RuntimeMonitoringCredentials};
use aws_config::{sts::AssumeRoleProvider, BehaviorVersion, Region, SdkConfig};

#[derive(Debug, Clone)]
pub struct Credentials {
    pub cloudwatch: Credential,
    pub dsql: Credential,
}

#[derive(Debug, Clone)]
pub struct Credential {
    pub account_id: String,
    pub role: String,
    pub region: Region,
}

impl Credential {
    fn is_lambda_environment() -> bool {
        std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok()
    }

    pub async fn sdk_config(&self) -> anyhow::Result<SdkConfig> {
        if Self::is_lambda_environment() {
            // Use the AssumeRoleProvider for Lambda environment
            // This connects to a role using its ARN
            tracing::info!("Using AssumeRoleProvider for Lambda environment");

            let base_config = aws_config::defaults(BehaviorVersion::latest())
                .region(self.region.clone())
                .load()
                .await;
            let role_arn = format!("arn:aws:iam::{}:role/{}", self.account_id, self.role);

            tracing::info!("Creating AssumeRoleProvider for role: {}", role_arn);

            let assume_role_provider = AssumeRoleProvider::builder(role_arn.clone())
                .region(self.region.clone())
                .session_name("PhotonTorpedo-Session")
                .configure(&base_config)
                .build()
                .await;

            tracing::info!("AssumeRoleProvider created, building final SDK config");

            Ok(aws_config::defaults(BehaviorVersion::latest())
                .credentials_provider(assume_role_provider)
                .region(self.region.clone())
                .load()
                .await)
        } else {
            // Use standard AWS credentials for local environment
            tracing::info!("Using standard AWS credentials for local environment");

            Ok(aws_config::defaults(BehaviorVersion::latest())
                .region(self.region.clone())
                .load()
                .await)
        }
    }
}

impl Default for Credentials {
    fn default() -> Self {
        let launch_config = LaunchConfig::default();
        Self::from_launch_config(&launch_config)
    }
}

impl Credentials {
    /// Create credentials from a launch configuration
    pub fn from_launch_config(config: &LaunchConfig) -> Self {
        let (_, dsql_cred, monitoring_creds) = config.to_credentials();

        Self {
            cloudwatch: monitoring_creds.prod, // Default to prod
            dsql: dsql_cred,
        }
    }

    /// Update credentials with new monitoring stage
    pub fn update_monitoring_stage(
        &mut self,
        stage: &str,
        monitoring_creds: &RuntimeMonitoringCredentials,
    ) -> anyhow::Result<()> {
        self.cloudwatch = match stage {
            "beta" => monitoring_creds.beta.clone(),
            "gamma" => monitoring_creds.gamma.clone(),
            "prod" => monitoring_creds.prod.clone(),
            _ => anyhow::bail!("Unknown monitoring stage: {}", stage),
        };
        Ok(())
    }

    pub fn cloudwatch_region(&mut self, region: String) {
        self.cloudwatch = Credential {
            account_id: self.cloudwatch.account_id.clone(),
            role: self.cloudwatch.role.clone(),
            region: aws_config::Region::new(region),
        };
    }
}
