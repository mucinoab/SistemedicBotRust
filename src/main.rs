use std::{
    env, thread,
    time::{Duration, Instant},
};

use deunicode::deunicode;
use futures::StreamExt;
use regex::{Regex, RegexSet};
use std::collections::BTreeMap;
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

    tokio::spawn(async move {
        connection.await.expect("Conexión a base de datos fallida.");
    });

    let map: BTreeMap<String, Persona> = client
        .query(
            "SELECT * FROM bot_claves UNION SELECT * FROM bot_internos;",
            &[],
        )
        .await?
        .iter()
        .map(|row| {
            (
                row.get::<usize, &str>(0).to_uppercase(),
                Persona {
                    generacion: match row.get(1) {
                        0 => String::from("N"),
                        _ => roman::to(row.get(1)).unwrap(),
                    },

                    nombre: row.get(2),

                    apellidos: row.get(3),
                },
            )
        })
        .collect();

    let re_claves: Regex = Regex::new(r"([Aa]\d{3}\*?)*([cC]\S{3})*").unwrap();

    let mut texto = String::with_capacity(348);
    lazy_static::initialize(&RE);

    let api = Api::new(&env::var("TOKEN").expect("Token no encontrado"));
    let mut stream = api.stream();

    info!("Datos procesados, listo para recibir querys.");
    while let Some(update) = stream.next().await {
        match update {
            Ok(update) => {
                if let UpdateKind::Message(message) = update.kind {
                    if let MessageKind::Text { ref data, .. } = message.kind {
                        let now = Instant::now();

                        match Comando::from(data) {
                            Comando::Clave => {
                                for cap in re_claves.find_iter(data) {
                                    if let Some((clave, datos)) =
                                        map.get_key_value(&cap.as_str().to_uppercase())
                                    {
                                        texto.push_str(&format!(
                                            "{}  {} {}, gen {}\n",
                                            clave,
                                            datos.nombre,
                                            datos.apellidos.split_whitespace().next().unwrap_or(""),
                                            datos.generacion,
                                        ));
                                    }
                                }
                            }

                            Comando::Nombre => {
                                for palabra in data.split_whitespace() {
                                    if palabra.len() > 2 {
                                        let palabra = &deunicode(palabra).to_lowercase();

                                        texto.push_str(
                                            &map.iter()
                                                .filter_map(|(clave, datos)| {
                                                    if deunicode(&datos.nombre)
                                                        .to_lowercase()
                                                        .contains(palabra)
                                                    {
                                                        Some(format!(
                                                            "{}  {}\n",
                                                            clave, datos.apellidos
                                                        ))
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect::<String>(),
                                        );
                                    }
                                }
                            }

                            Comando::Apellido => {
                                for palabra in data.split_whitespace() {
                                    if palabra.len() > 2 {
                                        let palabra = &deunicode(palabra).to_lowercase();

                                        texto.push_str(
                                            &map.iter()
                                                .filter_map(|(clave, datos)| {
                                                    if deunicode(&datos.apellidos)
                                                        .to_lowercase()
                                                        .contains(palabra)
                                                    {
                                                        Some(format!(
                                                            "{}  {}\n",
                                                            clave, datos.nombre
                                                        ))
                                                    } else {
                                                        None
                                                    }
                                                })
                                                .collect::<String>(),
                                        );
                                    }
                                }
                            }

                            Comando::Generacion => {
                                for palabra in data.split_whitespace() {
                                    if let Ok(gen) = palabra.parse::<i32>() {
                                        if let Some(gen_obj) = roman::to(gen) {
                                            texto.push_str(&format!("\nGen {}\n\n", gen_obj));
                                            if gen > 15 && gen < 34 {
                                                texto.push_str(
                                                    &map.iter()
                                                        .filter_map(|(clave, datos)| {
                                                            if datos.generacion == gen_obj {
                                                                Some(format!(
                                                                    "{}  {} {}\n",
                                                                    clave,
                                                                    datos.nombre,
                                                                    datos
                                                                        .apellidos
                                                                        .split_whitespace()
                                                                        .next()
                                                                        .unwrap_or("")
                                                                ))
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                        .collect::<String>(),
                                                );
                                            } else {
                                                texto.push_str(
                                                    "No tengo datos sobre esta generación. :( \n\n",
                                                );
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

            Err(e) => {
                error!("{}", e);
                thread::sleep(Duration::from_millis(1000));
            }
        }
    }

    Ok(())
}

fn responde(texto: &str, message: &Message, api: &Api) {
    if texto.is_empty() {
        api.spawn(message.chat.text(NO_ENCONTRE));
    } else {
        api.spawn(message.chat.text(texto));
    }
}

struct Persona {
    nombre: String,
    apellidos: String,
    generacion: String,
}

enum Comando {
    Clave,
    Nombre,
    Apellido,
    Generacion,
    Ayuda,
    Start,
    None,
}

impl From<&String> for Comando {
    fn from(item: &String) -> Self {
        let matches = RE.matches(item);

        if matches.matched(0) {
            Self::Clave
        } else if matches.matched(1) {
            Self::Nombre
        } else if matches.matched(2) {
            Self::Apellido
        } else if matches.matched(3) {
            Self::Generacion
        } else if matches.matched(4) {
            Self::Ayuda
        } else if matches.matched(5) {
            Self::Start
        } else {
            Self::None
        }
    }
}

lazy_static! {
    static ref RE: RegexSet =
        RegexSet::new(&["/[cC]", "/[nN]", "/[aA]", "/[gG]", "/[hH]", "/[sS]"]).unwrap();
}

static START: &str = r#"Para buscar...
- Clave usa /clave más las claves. 
- Generación entera /generacion más la generación.
- Nombre usa /nombre más los nombres.
- Apellido usa /apellido más los apellidos.

Búsquedas incluyen azules e internos. 

Ejemplo
/clave A101 A027 A010* cKGr 
/nombre Luis Karol
/generacion 33 32
/apellido Soriano

Para ayuda usa /help
Comparte con https://t.me/sistemedicbot"#;

static AYUDA: &str = r#"Para buscar por... 
- Clave usa /clave ó /c más las claves.
- Nombre usa /nombre ó /n más los nombres.
- Apellido usa /apellido ó /a más los apellidos.
- Generaciones enteras usa /generacion ó /g más las generaciones. 
Búsquedas incluyen azules e internos. 

Ejemplo
/g 32 33 19
/clave a101 A007 A010*
/c A342 A225 cKGr
/n Sam Pedro
/a castillo

Código Fuente: https://github.com/mucinoab/SistemedicBotRust"#;

static NO_ENTIENDO: &str = "No te entendí...\nIntenta de nuevo o usa \"/h\" para ayuda.";

static NO_ENCONTRE: &str = "No encontré lo que buscas...\nIntenta de nuevo.";
