use actix_web::web;

pub mod auth;
pub mod error;
pub mod handlers;
pub mod payload;
pub mod serde;
pub mod utils;
pub mod values;
pub mod views;

pub use self::serde::de;
pub use self::serde::ser;

pub trait IntoLegacyFormat {
    type Format;

    fn into_legacy_format(&self) -> Self::Format;
}

pub trait FromLegacyFormat {
    type Format;

    fn from_legacy_format(_: Self::Format) -> Self;
}

struct Error;

pub fn service_config(config: &mut web::ServiceConfig) {
    // Public routes
    config.service(web::resource("/").route(web::get().to(views::index)));
    config.service(web::resource("/su").route(web::get().to(views::script_url)));

    // Protected routes
    let auth_mw = auth::middleware::DeviceKeyMiddleware::device_key_md5(auth::validator);
    config.service(
        web::scope("")
            .wrap(auth_mw)
            .service(web::resource("/cv").route(web::get().to(handlers::change_settings::handler)))
            .service(web::resource("/jc").route(web::get().to(handlers::json_settings::handler)))
            .service(web::resource("/dp").route(web::get().to(handlers::delete_program::handler)))
            .service(web::resource("/cp").route(web::get().to(handlers::change_program::handler)))
            .service(web::resource("/cr").route(web::get().to(handlers::change_run_once::handler)))
            .service(web::resource("/mp").route(web::get().to(handlers::manual_program::handler)))
            .service(web::resource("/up").route(web::get().to(handlers::change_program_index::handler)))
            .service(web::resource("/jp").route(web::get().to(handlers::json_programs::handler)))
            .service(web::resource("/co").route(web::get().to(handlers::change_options::handler)))
            .service(web::resource("/jo").route(web::get().to(handlers::json_options::handler)))
            .service(web::resource("/sp").route(web::get().to(handlers::change_password::handler)))
            .service(web::resource("/js").route(web::get().to(handlers::json_status::handler)))
            .service(web::resource("/cm").route(web::get().to(handlers::change_manual::handler)))
            .service(web::resource("/cs").route(web::get().to(handlers::change_stations::handler)))
            .service(web::resource("/jn").route(web::get().to(handlers::json_stations::handler)))
            .service(web::resource("/je").route(web::get().to(handlers::json_stations_special::handler)))
            .service(web::resource("/jl").route(web::get().to(handlers::json_log::handler)))
            .service(web::resource("/dl").route(web::get().to(handlers::delete_log::handler)))
            .service(web::resource("/cu").route(web::get().to(handlers::change_script_url::handler)))
            .service(web::resource("/ja").route(web::get().to(handlers::json_all::handler))),
    );
}
