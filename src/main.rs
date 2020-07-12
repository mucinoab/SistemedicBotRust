use deunicode::deunicode;
use futures::StreamExt;
use regex::{Regex, RegexSet};
use std::collections::HashMap;
use std::convert::From;
use std::env;
use std::time::Instant;
use telegram_bot::Api;
use telegram_bot::*;
use tokio_postgres::{Error, NoTls};

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

#[tokio::main]
async fn main() -> Result<(), Error> {
    pretty_env_logger::init_timed();

    let (client, connection) = tokio_postgres::connect(
        &env::var("DATABASE").expect("Base de datos no encontrada o mal configurada."),
        NoTls,
    )
    .await?;

    tokio::spawn(async move {
        connection.await.expect("Conexión fallida.");
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
        .iter()
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

    let mut map: HashMap<String, &Filas> = HashMap::with_capacity(azules.len() + internos.len());

    for datos in azules.iter().chain(internos.iter()) {
        map.insert(datos.clave.to_uppercase(), datos);
    }

    let re_azules: Regex = Regex::new(r"[Aa]\d{3}\*?").unwrap();
    let re_internos: Regex = Regex::new(r"[cC]\S{3}").unwrap();

    let mut nombres_encontrados = String::with_capacity(300);
    let mut bandera: bool = false;

    let api = Api::new(&env::var("TOKEN").expect("Token no encontrado"));
    let mut stream = api.stream();

    while let Some(update) = stream.next().await {
        let update = update.unwrap();
        if let UpdateKind::Message(message) = update.kind {
            if let MessageKind::Text { ref data, .. } = message.kind {
                let now = Instant::now();

                let comando = Comando::from(data);

                match comando {
                    Comando::ClaveAzul => {
                        for cap in re_azules.captures_iter(data) {
                            let c = cap[0].to_uppercase();
                            if let Some(lineas) = map.get(&c) {
                                api.send(message.chat.text(format!(
                                    "{} es {} {}, generación {}.",
                                    c,
                                    lineas.nombre,
                                    lineas.apellidos.split(' ').next().unwrap(),
                                    lineas.generacion,
                                )))
                                .await
                                .unwrap();
                            } else {
                                api.send(message.chat.text(
                                    "Parece que no mencionaste a nadie conocido...\nIntenta de nuevo.",
                                ))
                                .await
                                .unwrap();
                            }
                        }
                    }

                    Comando::NombreAzul => {
                        for palabra in data.split(' ') {
                            if palabra.len() > 2 {
                                let palabra = &deunicode(palabra).to_lowercase();
                                for (clave, linea) in &map {
                                    if deunicode(&linea.nombre).to_lowercase().contains(palabra) {
                                        nombres_encontrados.push_str(&clave);
                                        nombres_encontrados.push_str(", ");
                                        bandera = true;
                                    }
                                }
                            }
                        }

                        if bandera {
                            api.send(message.chat.text(format!(
                                "Las siguientes claves tienen ese nombre {}.",
                                nombres_encontrados.trim_end_matches(", ")
                            )))
                            .await
                            .unwrap();
                            nombres_encontrados.clear();
                            bandera = false;
                        } else {
                            api.send(message.chat.text(
                                "Parece que no hay nadie con ese nombre...\nIntenta de nuevo.",
                            ))
                            .await
                            .unwrap();
                        }
                    }

                    Comando::ApellidoAzul => {
                        for palabra in data.split(' ') {
                            if palabra.len() > 2 {
                                let palabra = &deunicode(palabra).to_lowercase();
                                for (clave, linea) in &map {
                                    if deunicode(&linea.apellidos).to_lowercase().contains(palabra)
                                    {
                                        nombres_encontrados.push_str(&clave);
                                        nombres_encontrados.push_str(", ");
                                        bandera = true;
                                    }
                                }
                            }
                        }

                        if bandera {
                            api.send(message.chat.text(format!(
                                "Las siguientes claves tienen ese apellido {}.",
                                nombres_encontrados.trim_end_matches(", ")
                            )))
                            .await
                            .unwrap();
                            nombres_encontrados.clear();
                            bandera = false;
                        } else {
                            api.send(message.chat.text(
                                "Parece que no hay nadie con ese apellido...\nIntenta de nuevo.",
                            ))
                            .await
                            .unwrap();
                        }
                    }

                    Comando::ClaveInterno => {
                        for cap in re_internos.captures_iter(data) {
                            let c = cap[0].to_uppercase();
                            if let Some(lineas) = map.get(&c) {
                                api.send(message.chat.text(format!(
                                    "{} es {} {}, generación {}.",
                                    c,
                                    lineas.nombre,
                                    lineas.apellidos.split(' ').next().unwrap(),
                                    lineas.generacion,
                                )))
                                .await
                                .unwrap();
                            } else {
                                api.send(message.chat.text(
                                    "Parece que no mencionaste a nadie conocido...\nIntenta de nuevo.",
                                ))
                                .await
                                .unwrap();
                            }
                        }
                    }

                    Comando::NombreInterno => {
                        for palabra in data.split(' ') {
                            if palabra.len() > 2 {
                                let palabra = &deunicode(palabra).to_lowercase();
                                for (clave, linea) in &map {
                                    if deunicode(&linea.nombre).to_lowercase().contains(palabra) {
                                        nombres_encontrados.push_str(&clave);
                                        nombres_encontrados.push_str(", ");
                                        bandera = true;
                                    }
                                }
                            }
                        }

                        if bandera {
                            api.send(message.chat.text(format!(
                                "Las siguientes claves tienen ese nombre {}.",
                                nombres_encontrados.trim_end_matches(", ")
                            )))
                            .await
                            .unwrap();
                            nombres_encontrados.clear();
                            bandera = false;
                        } else {
                            api.send(message.chat.text(
                                "Parece que no hay nadie con ese nombre...\nIntenta de nuevo.",
                            ))
                            .await
                            .unwrap();
                        }
                    }

                    Comando::Ayuda => {
                        api.send(message.chat.text(AYUDA)).await.unwrap();
                    }

                    Comando::Start => {
                        api.send(message.chat.text(START)).await.unwrap();
                    }

                    Comando::None => {
                        api.send(
                            message.chat.text(
                                "No te entendí...\nIntenta de nuevo o usa \"/h\" para ayuda.",
                            ),
                        )
                        .await
                        .unwrap();
                    }
                };

                info!(
                    "{}: {} {:#?}",
                    &message.from.first_name,
                    data,
                    Instant::now().duration_since(now)
                );
            }
        }
    }
    Ok(())
}

struct Filas {
    clave: String,
    generacion: String,
    nombre: String,
    apellidos: String,
}

enum Comando {
    ClaveAzul,
    NombreAzul,
    ApellidoAzul,
    ClaveInterno,
    NombreInterno,
    Ayuda,
    Start,
    None,
}

impl From<&String> for Comando {
    fn from(item: &String) -> Self {
        lazy_static! {
            static ref RE: RegexSet =
                RegexSet::new(&[r"/c", r"/in", r"/n", r"/a", r"/ic", r"/h", r"/s",]).unwrap();
        }

        let matches = RE.matches(item);

        if matches.matched(0) {
            Self::ClaveAzul
        } else if matches.matched(1) {
            Self::NombreInterno
        } else if matches.matched(2) {
            Self::NombreAzul
        } else if matches.matched(3) {
            Self::ApellidoAzul
        } else if matches.matched(4) {
            Self::ClaveInterno
        } else if matches.matched(5) {
            Self::Ayuda
        } else if matches.matched(6) {
            Self::Start
        } else {
            Self::None
        }
    }
}

static START: &str = "Hola soy el SistemedicBot.\
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
                    \n\nPara ayuda usa /help";

static AYUDA: &str = "Para buscar por clave usa \"/clave\" ó \"/c\" más las claves a buscar.\
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
                    \nCódigo Fuente https://github.com/mucinoab/SistemedicBot/";
