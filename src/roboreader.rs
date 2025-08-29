use std::collections::HashMap;
use url::Url;
use reqwest::blocking::get;

pub struct RoboRules {
    rules_agents: Vec<String>,
    rules: HashMap<String, bool>
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct RoboReader {
    // Disallowed urls
    pub user: String,
    pub rules: HashMap<String, bool>,
    pub sitemap: String,
}

impl RoboReader {
    fn categorize_rules(parsed_url: &Url) -> Result<Vec<RoboRules>, Box<dyn std::error::Error>> {
        let robo_url = parsed_url.join("robots.txt")?;
        let request = get(robo_url.as_str())?;
        let body = request.text()?;

        let mut curr_ruleset = RoboRules {
            rules_agents: Vec::new(),
            rules: HashMap::new(),
        };
        let mut robo_rules: Vec<RoboRules> = Vec::new();
        for line in body.lines() {
            if line.starts_with("#") {
                continue;
            }
            else if line.is_empty() {
                robo_rules.push(curr_ruleset);
                curr_ruleset = RoboRules {
                    rules_agents: Vec::new(),
                    rules: HashMap::new(),
                };
                continue;
            }

            let parts: Vec<&str> = line.trim().splitn(2, ":").collect();
            
            if parts[0].trim() == "User-agent" {
                curr_ruleset.rules_agents.push(parts[1].trim().to_string());
            }
            else if parts[0].trim() == "Allow" || parts[0].trim() == "Disallow" {
                curr_ruleset.rules.insert(parts[1].trim().to_string(), parts[0].trim() == "Allow");
            }
        }

        Ok(robo_rules)
    }

    fn determine_ruleset(robo_rules: &[RoboRules], user: &str) -> Result<HashMap<String, bool>, Box<dyn std::error::Error>> {
        let mut candidate_user: String = "".to_string();
        let mut candidate: HashMap<String, bool> = HashMap::new();
        for robo_rule in robo_rules {
            for agent in &robo_rule.rules_agents {
                if agent == user {
                    candidate_user = agent.to_string();
                    candidate = robo_rule.rules.clone();
                }
                else if agent.starts_with(user) && candidate_user != user && agent.len() > candidate_user.len() {
                    candidate_user = agent.to_string();
                    candidate = robo_rule.rules.clone();
                }
                else if candidate_user != user && !candidate_user.starts_with(user) && agent == "*" {
                    candidate_user = agent.to_string();
                    candidate = robo_rule.rules.clone();
                }
            }
        }
        if candidate_user == "" {
            let empty_rules: HashMap<String, bool> = HashMap::new();
            return Ok(empty_rules);
        }

        Ok(candidate)
    }

    fn get_sitemap(parsed_url: &Url) -> Result<String, Box<dyn std::error::Error>> {
        let robo_url = parsed_url.join("robots.txt")?;
        let request = get(robo_url.as_str())?;
        let body = request.text()?;

        for line in body.lines() {
            let parts: Vec<&str> = line.splitn(2, ":").collect();
            if parts[0].trim() == "Sitemap" {
                return Ok(parts[1].to_string())
            }
        }

        Ok("".to_string())
    }

    pub fn new(url: &str, user_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let parsed_url = Url::parse(url)?;
        let rules_categorized = Self::categorize_rules(&parsed_url)?;
        Ok(RoboReader {
            user: user_name.to_string(),
            rules: Self::determine_ruleset(&rules_categorized, user_name)?,
            sitemap: Self::get_sitemap(&parsed_url)?,
        })
    }
}