use dotenv::dotenv;
use std::env;

pub const NEPHRITE_ROOT: &'static str = "NEPHRITE_ROOT";
pub const NEPHRITE_DB_URL: &'static str = "NEPHRITE_DB_URL";
pub const NEPHRITE_ST_URL: &'static str = "NEPHRITE_ST_URL";
pub const NEPHRITE_ST_TOKEN: &'static str = "NEPHRITE_ST_TOKEN";
pub const NEPHRITE_CUT_PORT: &'static str = "NEPHRITE_CUT_PORT";

#[derive(Debug, Copy, Clone)]
pub struct Conf {
}

impl Conf {
    pub fn root(&self) -> String {
        env::var(NEPHRITE_ROOT)
            .expect(&format!("{} must be set", NEPHRITE_ROOT))
            .to_string()
    }

    pub fn db_url(&self) -> String {
        env::var(NEPHRITE_DB_URL)
            .expect(&format!("{} must be set", NEPHRITE_DB_URL))
            .to_string()
    }

    pub fn cut_port(&self) -> u32 {
        env::var(NEPHRITE_CUT_PORT)
            .expect(&format!("{} must be set", NEPHRITE_DB_URL))
            .parse::<u32>()
            .unwrap()
    }

    pub fn read() -> Conf {
        dotenv().ok();

        /*
        let st_url = env::var(NEPHRITE_ST_URL)
            .expect(&format!("{} must be set", NEPHRITE_ST_URL))
            .to_string();
        let st_token = env::var(NEPHRITE_ST_TOKEN)
            .expect(&format!("{} must be set", NEPHRITE_ST_TOKEN))
            .to_string();
         */
        Conf {}
    }
}
