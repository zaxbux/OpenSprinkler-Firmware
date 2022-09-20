pub mod handlers;
pub mod legacy;
pub mod views;

use std::sync::{mpsc, Arc, Mutex};

use actix_web::{dev::ServerHandle, middleware, web, App, HttpServer};
use handlebars::Handlebars;

use crate::opensprinkler::Controller;

//pub type OpenSprinklerMutex = web::Data<Arc<Mutex<OpenSprinkler>>>;

pub async fn run_app(tx: mpsc::Sender<ServerHandle>, open_sprinkler: Arc<Mutex<Controller>>) -> std::io::Result<()> {
    // Handlebars uses a repository for the compiled templates. This object must be
    // shared between the application threads, and is therefore passed to the
    // Application Builder as an atomic reference-counted pointer.
    let mut handlebars = Handlebars::new();
    handlebars.register_templates_directory(".html", "./static/templates").unwrap();
    let handlebars_ref = web::Data::new(handlebars);

    let open_sprinkler_ref = web::Data::new(open_sprinkler);

    // srv is server controller type, `dev::Server`
    let server = HttpServer::new(move || {
        App::new()
            // Error handlers
            .wrap(handlers::errors())
            // enable logger
            .wrap(middleware::Logger::default())
            // handlebars
            .app_data(handlebars_ref.clone())
            // OpenSprinkler
            .app_data(open_sprinkler_ref.clone())
            .configure(service_config)
    })
    .bind(("127.0.0.1", 8080))?
    .workers(2)
    .run();

    // Send server handle back to the main thread
    let _ = tx.send(server.handle());

    server.await
}

fn service_config(config: &mut web::ServiceConfig) {
    // Legacy API
    legacy::service_config(config);

    // Default service
    config.default_service(web::to(handlers::default));
}
