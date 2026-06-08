use malbox_config::core::{DATABASE_NAME, DatabaseConfig};
pub use sqlx::Error;
pub use sqlx::PgPool;
pub use sqlx::error::DatabaseError;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

pub mod error;
pub mod repositories;

/// Connect to the daemon's database and apply the embedded migrations.
/// Creating the database itself is provisioning and deliberately not done
/// here - the installer and dev tooling own that - so a missing database
/// fails with instructions. Errors are returned rather than panicking so
/// daemon startup can fail with an actionable message.
pub async fn init_database(
    config: &DatabaseConfig,
) -> Result<sqlx::Pool<sqlx::Postgres>, sqlx::Error> {
    // Configs from before the [database] section was split into discrete
    // fields put a full connection URL here; fail with directions instead
    // of trying to resolve the URL as a hostname.
    if config.host.contains("://") {
        return Err(sqlx::Error::Configuration(
            format!(
                "database.host holds a connection URL ('{}'); newer configs \
                 use discrete host/port/user/password fields",
                config.host
            )
            .into(),
        ));
    }

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect_with(connect_options(config))
        .await
        .map_err(|err| {
            // 3D000 invalid_catalog_name: the database doesn't exist.
            if error_code(&err).as_deref() == Some("3D000") {
                sqlx::Error::Configuration(
                    format!(
                        "database \"{DATABASE_NAME}\" does not exist on {}:{} - \
                         run 'malboxctl install', or create it manually: \
                         CREATE DATABASE \"{DATABASE_NAME}\"",
                        config.host, config.port
                    )
                    .into(),
                )
            } else {
                err
            }
        })?;

    sqlx::migrate!().run(&db).await?;

    Ok(db)
}

/// Build connection options from the discrete config fields.
///
/// An omitted user resolves like psql: `PGUSER`, then the OS user. sqlx's
/// own fallback cannot be trusted here - 0.9 builds `whoami` without its
/// `std` feature, so the default username degrades to the stub value
/// "anonymous", a role that exists on no server.
fn connect_options(config: &DatabaseConfig) -> PgConnectOptions {
    let mut options = PgConnectOptions::new()
        .host(&config.host)
        .port(config.port)
        .database(DATABASE_NAME);

    let user = config
        .user
        .clone()
        .or_else(|| std::env::var("PGUSER").ok().filter(|user| !user.is_empty()))
        .or_else(os_user);
    if let Some(user) = user {
        options = options.username(&user);
    }
    if let Some(password) = &config.password {
        options = options.password(password);
    }

    options
}

/// The invoking OS user, mirroring libpq's default-username lookup.
fn os_user() -> Option<String> {
    ["USER", "LOGNAME"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|user| !user.is_empty()))
}

fn error_code(err: &sqlx::Error) -> Option<String> {
    match err {
        sqlx::Error::Database(db_err) => db_err.code().map(|code| code.into_owned()),
        _ => None,
    }
}
