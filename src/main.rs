use dotenv::dotenv;
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
//use postgres::{Client, NoTls};
use postgres_openssl::MakeTlsConnector;
use regex::Regex;
use std::env;
use std::time::Instant;
use telegram_bot::Api;
use tokio_postgres::{Error, NoTls};

#[macro_use]
extern crate lazy_static;

#[derive(Debug)]
struct Filas {
    clave: String,
    generacion: String,
    nombre: String,
    apellidos: String,
}

#[derive(Debug)]
enum Comando {
    ClaveAzul,
    NombreAzul,
    ApellidoAzul,
    ClaveInterno,
    NombreInterno,
    Ayuda,
    Start,
}
use futures::StreamExt;
use telegram_bot::*;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
    builder.set_verify(SslVerifyMode::NONE);
    let connector = MakeTlsConnector::new(builder.build());
    let (client, connection) = tokio_postgres::connect(
        &env::var("DATABASE_URL").expect("database_url no encontrada"),
        connector,
    )
    .await?;

    //let (client, connection) = tokio_postgres::connect("host=localhost user=bruno", NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("connection error: {}", e);
        }
    });

    let mut numero_de_filas: i64 = client
        .query_one("SELECT count(*) n from bot_claves", &[])
        .await?
        .get("n");

    let mut azules: Vec<Filas> = Vec::with_capacity(numero_de_filas as usize);

    numero_de_filas = client
        .query_one("SELECT count(*) n from bot_internos", &[])
        .await?
        .get("n");

    let mut internos: Vec<Filas> = Vec::with_capacity(numero_de_filas as usize);

    for row in client
        .query(
            "SELECT clave, generacion, nombre, apellidos from bot_claves",
            &[],
        )
        .await?
        .into_iter()
    {
        let clave: String = row.get(0);
        let generacion = match row.get(1) {
            0 => String::from("N"),
            _ => roman::to(row.get(1)).unwrap(),
        };
        let nombre: String = row.get(2);
        let apellidos: String = row.get(3);
        azules.push(Filas {
            clave,
            generacion,
            nombre,
            apellidos,
        });
    }

    for row in client
        .query(
            "select clave, generacion, nombre, apellidos from bot_internos",
            &[],
        )
        .await?
        .into_iter()
    {
        let clave: String = row.get(0);
        let generacion = match row.get(1) {
            0 => String::from("n"),
            _ => roman::to(row.get(1)).unwrap(),
        };
        let nombre: String = row.get(2);
        let apellidos: String = row.get(3);
        internos.push(Filas {
            clave,
            generacion,
            nombre,
            apellidos,
        });
    }

    lazy_static! {
        static ref PATRON_AZULES: Regex = Regex::new(r"[Aa]\d{3}\*?").unwrap();
        static ref PATRON_INTERNOS: Regex = Regex::new(r"[cC]\S{3}").unwrap();
    }

    let api = Api::new(&env::var("TOKEN").expect("token no encontrada"));
    let mut stream = api.stream();
    while let Some(update) = stream.next().await {
        let update = update.unwrap();
        if let UpdateKind::Message(message) = update.kind {
            if let MessageKind::Text { ref data, .. } = message.kind {
                println!("<{}>: {}", &message.from.first_name, data);

                let now = Instant::now();

                let mut comando = Comando::ClaveAzul;

                if data.contains("/c") {
                    comando = Comando::ClaveAzul;
                } else if data.contains("/n") {
                    comando = Comando::NombreAzul;
                } else if data.contains("/a") {
                    comando = Comando::ApellidoAzul;
                } else if data.contains("/ic") {
                    comando = Comando::ClaveInterno;
                } else if data.contains("/in") {
                    comando = Comando::NombreInterno;
                } else if data.contains("/h") {
                    comando = Comando::Ayuda;
                } else if data.contains("/s") {
                    comando = Comando::Start;
                } else {
                    api.send(message.text_reply(String::from(
                        "No te entendí...\nIntenta de nuevo o usa \"/h\" para ayuda.",
                    )))
                    .await
                    .unwrap();
                }

                match comando {
                    Comando::ClaveAzul => {
                        for cap in PATRON_AZULES.captures_iter(data) {
                            let c = cap[0].to_uppercase();
                            for lineas in &azules {
                                if c == lineas.clave {
                                    api.send(message.chat.text(format!(
                                        "{} es {} {}, generación {}.",
                                        c,
                                        lineas.nombre,
                                        lineas.apellidos.split(' ').next().unwrap(),
                                        lineas.generacion,
                                    )))
                                    .await
                                    .unwrap();
                                }
                            }
                        }
                    }

                    Comando::NombreAzul => {}

                    Comando::ApellidoAzul => {}

                    Comando::ClaveInterno => {
                        for cap in PATRON_INTERNOS.captures_iter(data) {
                            let c = cap[0].to_uppercase();
                            for lineas in &internos {
                                if c == lineas.clave.to_uppercase() {
                                    api.send(message.text_reply(format!(
                                        "{} es {} {}, generación {}.",
                                        c,
                                        lineas.nombre,
                                        lineas.apellidos.split(' ').next().unwrap(),
                                        lineas.generacion,
                                    )))
                                    .await
                                    .unwrap();
                                }
                            }
                        }
                    }

                    Comando::NombreInterno => {}

                    Comando::Ayuda => {
                        api.send(message.text_reply(String::from(
                            "Para buscar por clave usa \"/clave\" ó \"/c\" más las claves a buscar.\
                            \nPara buscar por nombre usa \"/nombre\" ó \"/n\" más los nombres a buscar.\
                            \nPara buscar por apellido usa \"/apellido\"  ó \"/a\"más los apellidos a buscar.\
                            \nPara buscar internos por clave usa \"/iclave\" ó \"/ic\" más las claves.\
                            \nPara buscar internos por nombre usa \"/inombre\" ó \"/in\" más los nombres a buscar.\
                            \n\nEjemplo\
                            \n\"/clave A101 A027 A007 A010*\"\
                            \n\"/c a342\"\
                            \n\"/iclave cKGr\"\
                            \n\"/in Sam\"\
                            \n\"/nombre Luis\"\
                            \n\"/apellido Castillo\"\
                            \n\nComparte https://t.me/sistemedicbot\
                            \nCódigo Fuente https://github.com/mucinoab/SistemedicBot/"
                        ))).await.unwrap();
                    }

                    Comando::Start => {
                        api.send(message.text_reply(String::from(
                            "Hola soy el SistemedicBot.\
                            \nPara buscar por clave usa \"/clave\" más las claves a buscar.\
                            \nPara buscar por nombre usa \"/nombre\" más los nombres a buscar.\
                            \nPara buscar por apellido usa \"/apellido\" más los apellidos a buscar.\
                            \nPara buscar interno por clave usa \"/iclave\" más las claves.\
                            \nPara buscar internos por nombre usa \"/inombre\" más los nombres a buscar.\
                            \n\nEjemplo\
                            \n\"/clave A101 A027 A007 A001 A010* A010\"\
                            \n\"/iclave cKGr\"\
                            \n\"/inombre Karol\"\
                            \n\"/nombre Luis\"\
                            \n\"/apellido Castillo\"\
                            \n\nPara ayuda usa /help"
                        ))).await.unwrap();
                    }
                };
                println!("{:#?}", Instant::now().duration_since(now));
            }
        }
    }
    Ok(())
}
