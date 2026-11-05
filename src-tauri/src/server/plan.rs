//! ResinCraft Relay's paid plans (relay protocol 11): what every signed-in Microsoft account has, what the relay sells,
//! and buying, changing, cancelling and paying from the launcher's own payment form (Stripe.js in the renderer; card
//! details go from Stripe's fields straight to Stripe, never through here).
//!
//! Everything is per Minecraft account: a player with several accounts sees each one's plan and picks which one buys.
//! While the relay's admin has payments off, nothing is for sale and the launcher shows nothing to buy.

use anyhow::Result;
use resincraft_relay::agent::Connection;
use resincraft_relay::proto::{PayStep, PlanInfo};
use serde::Serialize;

use crate::core::text::Text;
use crate::player::accounts::{self, MicrosoftAccount};

use super::relay;

/// One signed-in account's plan.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPlan {
    pub account: String,
    pub account_name: String,
    /// None: the relay couldn't be asked (`error`), or is too old for plans (`unsupported`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanInfo>,
    pub unsupported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Text>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plans {
    /// This build has a ResinCraft Relay.
    pub available: bool,
    pub accounts: Vec<AccountPlan>,
}

async fn owner(account: &str) -> Result<MicrosoftAccount> {
    accounts::microsoft_accounts().await.into_iter().find(|a| relay::same_account(&a.uuid, account)).ok_or_else(relay::needs_microsoft)
}

async fn connect(account: &str) -> Result<Connection> {
    relay::connect_for_list(&owner(account).await?).await
}

/// Every signed-in Microsoft account's plan.
pub async fn plans() -> Result<Plans> {
    if relay::settings().is_none() {
        return Ok(Plans { available: false, accounts: Vec::new() });
    }
    let mut out = Vec::new();
    for account in accounts::microsoft_accounts().await {
        let mut entry = AccountPlan { account: account.uuid.clone(), account_name: account.username.clone(), plan: None, unsupported: false, error: None };
        match relay::connect_for_list(&account).await {
            Ok(conn) if !conn.has_plans() => entry.unsupported = true,
            Ok(mut conn) => match conn.plan().await {
                Ok(plan) => entry.plan = Some(plan),
                Err(e) => entry.error = Some(relay::as_text(&relay::describe(&e, ""))),
            },
            Err(e) => entry.error = Some(relay::as_text(&e)),
        }
        out.push(entry);
    }
    Ok(Plans { available: true, accounts: out })
}

pub async fn buy(account: &str, product: &str, yearly: bool) -> Result<PayStep> {
    connect(account).await?.plan_buy(product, yearly).await.map_err(|e| relay::describe(&e, ""))
}

pub async fn change(account: &str, sub: &str, product: &str, yearly: bool) -> Result<PayStep> {
    connect(account).await?.plan_change(sub, product, yearly).await.map_err(|e| relay::describe(&e, ""))
}

pub async fn cancel(account: &str, sub: &str, resume: bool) -> Result<PlanInfo> {
    connect(account).await?.plan_cancel(sub, resume).await.map_err(|e| relay::describe(&e, ""))
}

pub async fn pay(account: &str, sub: &str) -> Result<PayStep> {
    connect(account).await?.plan_pay(sub).await.map_err(|e| relay::describe(&e, ""))
}

pub async fn refresh(account: &str, sub: &str) -> Result<PlanInfo> {
    connect(account).await?.plan_refresh(sub).await.map_err(|e| relay::describe(&e, ""))
}
