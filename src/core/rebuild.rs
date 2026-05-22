use crate::core::init::build_and_write_config;
use crate::core::prompt::{prompt_init_options, InitOptions};
use crate::error::Result;

use super::config;

pub fn run(user: Option<&str>) -> Result<()> {
    let options = match user {
        Some(user) => prompt_init_options(user, None)?,
        None => {
            let config_path = config::default_config_path()?;
            let config = config::read_config(&config_path)?;
            InitOptions {
                user_name: config.user_name,
                user_email: config.user_email,
                ssh_key_path: config.ssh_key_path,
            }
        }
    };

    build_and_write_config(&options)
}
