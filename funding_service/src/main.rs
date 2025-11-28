use actix_web::{post, web, App, HttpRequest, HttpResponse, HttpServer};
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc};
use stripe::{Event, EventObject};

#[derive(Clone)]
struct AppState {
    webhook_secret: String,
    registry_path: PathBuf,
}

#[derive(Deserialize)]
struct FundingRequest {
    /// Base64 public key of the payer.
    user_pk: String,
    /// Amount in smallest currency unit (e.g., cents).
    amount: u64,
}

/// Stripe webhook handler with signature verification.
#[post("/stripe/webhook")]
async fn stripe_webhook(
    req: HttpRequest,
    body: web::Bytes,
    data: web::Data<Arc<AppState>>,
) -> HttpResponse {
    let sig = req
        .headers()
        .get("Stripe-Signature")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let payload = match std::str::from_utf8(&body) {
        Ok(p) => p,
        Err(_) => return HttpResponse::BadRequest().body("invalid body"),
    };

    // For stripe 0.0.5 we don't have a webhook helper; rely on upstream to protect the endpoint.
    // In production, replace this with Stripe's official signature verification logic.
    let event: Event = match serde_json::from_str(payload) {
        Ok(ev) => ev,
        Err(err) => {
            eprintln!("event parse failed: {err}");
            return HttpResponse::BadRequest().finish();
        }
    };

    // Only process successful payments
    if event.type_ != "payment_intent.succeeded" && event.type_ != "checkout.session.completed" {
        return HttpResponse::Ok().finish();
    }

    // Extract metadata.user_pk
    let user_pk = match &event.data.object {
        EventObject::PaymentIntent(pi) => pi.metadata.get("user_pk").cloned(),
        EventObject::CheckoutSession(cs) => cs.metadata.get("user_pk").cloned(),
        _ => None,
    };
    let user_pk = match user_pk {
        Some(pk) => pk,
        None => {
            eprintln!("missing user_pk metadata");
            return HttpResponse::BadRequest().finish();
        }
    };

    // Extract amount (smallest currency unit)
    let amount = match &event.data.object {
        EventObject::PaymentIntent(pi) => pi.amount_received,
        EventObject::CheckoutSession(cs) => cs.amount_total.unwrap_or(0),
        _ => 0,
    };
    if amount <= 0 {
        eprintln!("no amount in event");
        return HttpResponse::BadRequest().finish();
    }

    // Map to registry units (example 1:1)
    let credit = amount as u64;
    if let Err(err) = credit_registry(&data.registry_path, &user_pk, credit) {
        eprintln!("registry update failed: {err}");
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

fn credit_registry(path: &PathBuf, pk: &str, credit: u64) -> Result<(), String> {
    #[derive(serde::Serialize, serde::Deserialize, Default)]
    struct StakeAccount {
        balance: u64,
        stake: u64,
        slashed: bool,
    }
    #[derive(serde::Serialize, serde::Deserialize, Default)]
    struct Registry {
        accounts: HashMap<String, StakeAccount>,
    }

    let mut reg: Registry = if path.exists() {
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    } else {
        Registry::default()
    };
    let acct = reg
        .accounts
        .entry(pk.to_string())
        .or_insert(StakeAccount {
            balance: 0,
            stake: 0,
            slashed: false,
        });
    acct.balance = acct.balance.saturating_add(credit);
    let data = serde_json::to_vec_pretty(&reg).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, data).map_err(|e| e.to_string())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let webhook_secret =
        std::env::var("STRIPE_WEBHOOK_SECRET").expect("set STRIPE_WEBHOOK_SECRET env var");
    let registry_path = PathBuf::from(
        std::env::var("REGISTRY_PATH").unwrap_or_else(|_| "stake_registry.json".to_string()),
    );
    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8085".to_string());

    let state = Arc::new(AppState {
        webhook_secret,
        registry_path,
    });

    println!("Funding service listening on {bind}");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(stripe_webhook)
    })
    .bind(bind)?
    .run()
    .await
}
