use crate::aws::credentials::Credential;
use aws_config::Region;
use serde::{Deserialize, Serialize};

/// AWS regions available for configuration
pub const AWS_REGIONS: &[&str] = &[
    "us-east-1",
    "us-east-2",
    "us-west-1",
    "us-west-2",
    "ca-central-1",
    "eu-west-1",
    "eu-west-2",
    "eu-west-3",
    "eu-central-1",
    "eu-north-1",
    "ap-northeast-1",
    "ap-northeast-2",
    "ap-southeast-1",
    "ap-southeast-2",
    "ap-south-1",
    "sa-east-1",
];

/// AWS IAM roles available for configuration
pub const AWS_ROLES: &[&str] = &["Admin", "Bedrock-Access", "ReadOnly"];

/// Launch-time configuration - set once, rarely changed
/// Contains credentials and static endpoints needed to initialize the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchConfig {
    pub dsql_credential: LaunchCredential,
    pub monitoring_credentials: MonitoringCredentials,
    pub dsql_cluster_endpoint: String,
}

/// Simplified credential structure for TOML serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchCredential {
    pub account_id: String,
    pub role: String,
    pub region: String,
}

/// Monitoring credentials for different environments
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringCredentials {
    pub beta: LaunchCredential,
    pub gamma: LaunchCredential,
    pub prod: LaunchCredential,
}

/// Runtime configuration - changed frequently via UI dropdowns
/// Contains settings that users regularly adjust during operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub monitoring_stage: String, // beta/gamma/prod
    pub monitoring_region: String,
}

impl Default for LaunchConfig {
    /// Default configuration using mike.toml values
    fn default() -> Self {
        Self {
            dsql_credential: LaunchCredential {
                account_id: "603014702751".to_string(),
                role: "Bedrock-Access".to_string(),
                region: "us-west-2".to_string(),
            },
            monitoring_credentials: MonitoringCredentials {
                beta: LaunchCredential {
                    account_id: "331022015435".to_string(),
                    role: "ReadOnly".to_string(),
                    region: "us-east-1".to_string(),
                },
                gamma: LaunchCredential {
                    account_id: "322556739396".to_string(),
                    role: "ReadOnly".to_string(),
                    region: "us-east-1".to_string(),
                },
                prod: LaunchCredential {
                    account_id: "905418122172".to_string(),
                    role: "ReadOnly".to_string(),
                    region: "us-east-1".to_string(),
                },
            },
            dsql_cluster_endpoint: "euabuecic5ekalnoi4dukbczk4.dsql.us-west-2.on.aws".to_string(),
        }
    }
}

impl LaunchCredential {
    /// Convert LaunchCredential to runtime Credential
    pub fn to_credential(&self) -> Credential {
        Credential {
            account_id: self.account_id.clone(),
            role: self.role.clone(),
            region: Region::new(self.region.clone()),
        }
    }
}

/// Runtime monitoring credentials structure (different from serializable one)
#[derive(Debug, Clone)]
pub struct RuntimeMonitoringCredentials {
    pub beta: Credential,
    pub gamma: Credential,
    pub prod: Credential,
}

impl LaunchConfig {
    /// Convert LaunchConfig to individual Credentials for AWS client initialization
    pub fn to_credentials(&self) -> (Credential, Credential, RuntimeMonitoringCredentials) {
        let monitoring_creds = RuntimeMonitoringCredentials {
            beta: self.monitoring_credentials.beta.to_credential(),
            gamma: self.monitoring_credentials.gamma.to_credential(),
            prod: self.monitoring_credentials.prod.to_credential(),
        };

        // Return a dummy credential for bedrock (first position) since it's no longer used
        let dummy_cred = self.dsql_credential.to_credential();

        (
            dummy_cred,
            self.dsql_credential.to_credential(),
            monitoring_creds,
        )
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            monitoring_stage: "prod".to_string(),
            monitoring_region: "us-east-1".to_string(),
        }
    }
}
