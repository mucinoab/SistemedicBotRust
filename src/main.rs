use deunicode::deunicode;
use futures::StreamExt;
use regex::{Regex, RegexSet};
use std::{collections::HashMap, env, time::Instant};
use telegram_bot::*;
use tokio_postgres::{Error, NoTls};

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;

#[tokio::main]
async fn main() -> Result<(), Error> {
    log4rs::init_file("log_config.yml", Default::default()).expect("No se pudo iniciar Log");

    info!("Iniciando...");
    let (client, connection) = tokio_postgres::connect(
        &env::var("DATABASE").expect("Base de datos no encontrada o mal configurada."),
        NoTls,
    )
    .await?;

    info!("Conectando a base de datos...");
    tokio::spawn(async move {
        connection.await.expect("Conexión fallida.");
    });

    let numero_de_registros = client
        .query_one(
            "SELECT ((SELECT COUNT(*) FROM bot_claves) + (SELECT COUNT(*) FROM bot_internos))",
            &[],
        )
        .await?;

    let mut map: HashMap<String, Filas> =
        HashMap::with_capacity(numero_de_registros.get::<usize, i64>(0) as usize);

    for row in client
        .query(
            "SELECT clave, generacion, nombre, apellidos from bot_claves",
            &[],
        )
        .await?
        .iter()
    {
        map.insert(
            row.get(0),
            Filas {
                clave: row.get(0),
                generacion: match row.get(1) {
                    0 => String::from("N"),
                    _ => roman::to(row.get(1))
                        .expect("Error al convertir generación a número romano"),
                },
                nombre: row.get(2),
                apellidos: row.get(3),
            },
        );
    }

    for row in client
        .query(
            "select clave, generacion, nombre, apellidos from bot_internos",
            &[],
        )
        .await?
        .iter()
    {
        map.insert(
            row.get::<usize, String>(0).to_uppercase(),
            Filas {
                clave: row.get(0),
                generacion: match row.get(1) {
                    0 => String::from("N"),
                    _ => roman::to(row.get(1))
                        .expect("Error al convertir generación a número romano"),
                },
                nombre: row.get(2),
                apellidos: row.get(3),
            },
        );
    }

    let re_azules: Regex = Regex::new(r"[Aa]\d{3}\*?").expect("Error al compilar Regex");
    let re_internos: Regex = Regex::new(r"[cC]\S{3}").expect("Error al compilar Regex");

    let mut texto = String::with_capacity(348);

    let api = Api::new(&env::var("TOKEN").expect("Token no encontrado"));
    let mut stream = api.stream();

    info!("Datos procesados, listo para recibir querys");
    while let Some(update) = stream.next().await {
        match update {
            Ok(update) => {
                if let UpdateKind::Message(message) = update.kind {
                    if let MessageKind::Text { ref data, .. } = message.kind {
                        let now = Instant::now();

                        match Comando::from(data) {
                            Comando::ClaveAzul => {
                                for cap in re_azules.captures_iter(data) {
                                    if let Some(linea) = map.get(&cap[0].to_uppercase()) {
                                        texto.push_str(&format!(
                                            "{} es {} {}, generación {}.\n",
                                            linea.clave,
                                            linea.nombre,
                                            linea
                                                .apellidos
                                                .split_whitespace()
                                                .next()
                                                .expect("No se pudo separar apellidos"),
                                            linea.generacion,
                                        ));
                                    }
                                }
                            }

                            Comando::NombreAzul => {
                                for palabra in data.split_whitespace() {
                                    if palabra.len() > 2 {
                                        let palabra = &deunicode(palabra).to_lowercase();
                                        for (clave, linea) in &map {
                                            if deunicode(&linea.nombre)
                                                .to_lowercase()
                                                .contains(palabra)
                                            {
                                                texto.push_str(&format!("{}, ", clave));
                                            }
                                        }
                                    }
                                }
                            }

                            Comando::ApellidoAzul => {
                                for palabra in data.split_whitespace() {
                                    if palabra.len() > 2 {
                                        let palabra = &deunicode(palabra).to_lowercase();
                                        for linea in map.values() {
                                            if deunicode(&linea.apellidos)
                                                .to_lowercase()
                                                .contains(palabra)
                                            {
                                                texto.push_str(&format!("{}, ", linea.clave));
                                            }
                                        }
                                    }
                                }
                            }

                            Comando::ClaveInterno => {
                                texto.push_str(GEN_ACTUAL);
                                for cap in re_internos.captures_iter(data) {
                                    if let Some(linea) = map.get(&cap[0].to_uppercase()) {
                                        texto.push_str(&format!(
                                            "{} es {} {}.\n",
                                            linea.clave,
                                            linea.nombre,
                                            linea
                                                .apellidos
                                                .split_whitespace()
                                                .next()
                                                .expect("Error al separar apellidos"),
                                        ));
                                    }
                                }
                            }

                            Comando::NombreInterno => {
                                texto.push_str(GEN_ACTUAL);

                                for palabra in data.split_whitespace() {
                                    if palabra.len() > 2 {
                                        let palabra = &deunicode(palabra).to_lowercase();
                                        for linea in map.values() {
                                            if deunicode(&linea.nombre)
                                                .to_lowercase()
                                                .contains(palabra)
                                                && !linea.clave.ends_with(char::is_numeric)
                                            {
                                                texto.push_str(&format!(
                                                    "{} es {} {}.\n",
                                                    linea.clave,
                                                    linea.nombre,
                                                    linea
                                                        .apellidos
                                                        .split_whitespace()
                                                        .next()
                                                        .expect("Error al separar apellidos"),
                                                ));
                                            }
                                        }
                                    }
                                }
                            }

                            Comando::Ayuda => {
                                texto.push_str(AYUDA);
                            }

                            Comando::Start => {
                                texto.push_str(START);
                            }

                            Comando::None => {
                                texto.push_str(NO_ENTIENDO);
                            }
                        };

                        responde(&texto, &message, &api);

                        info!(
                            "{}: {:#?} {:#?}",
                            &message.from.first_name,
                            &data,
                            Instant::now().duration_since(now)
                        );

                        texto.clear();
                    }
                }
            }
            Err(e) => error!("{}", e),
        }
    }

    Ok(())
}

fn responde(texto: &String, message: &Message, api: &Api) {
    if texto.is_empty() {
        api.spawn(message.chat.text(NO_ENCONTRE));
    } else {
        api.spawn(message.chat.text(texto.trim_end_matches(", ")));
    }
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
            static ref RE: RegexSet = RegexSet::new(&[
                r"/[cC]",
                r"/[inIN]{2}",
                r"/[nN]",
                r"/[aA]",
                r"/[icIC]{2}",
                r"/[hH]",
                r"/[sS]",
            ])
            .unwrap();
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

static GEN_ACTUAL: &str = "Gen XXXIII\n\n";
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
                    \n\nPara ayuda usa /help\
                    \nComparte https://t.me/sistemedicbot";
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
                    \n\"/apellido Castillo\"\n\
                    \nCódigo Fuente https://github.com/mucinoab/SistemedicBotRust/";
static NO_ENTIENDO: &str = "No te entendí...\nIntenta de nuevo o usa \"/h\" para ayuda.";
static NO_ENCONTRE: &str = "No encontré lo que buscas...\nIntenta de nuevo.";
