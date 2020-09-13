use std::{
    env, thread,
    time::{Duration, Instant},
};

use deunicode::deunicode;
use futures::StreamExt;
use hashbrown::HashMap;
use rayon::prelude::*;
use regex::{Regex, RegexSet};
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
        connection.await.expect("Conexión a base de datos fallida.");
    });

    let numero_de_registros = client
        .query_one(
            "SELECT ((SELECT COUNT(*) FROM bot_claves) + (SELECT COUNT(*) FROM bot_internos))",
            &[],
        )
        .await?
        .get::<usize, i64>(0) as usize;

    let mut map = HashMap::with_capacity(numero_de_registros);

    for row in client
        .query(
            "SELECT clave, generacion, nombre, apellidos 
            FROM (SELECT * from bot_claves UNION SELECT * from bot_internos)x;",
            &[],
        )
        .await?
        .iter()
    {
        map.insert(
            row.get::<usize, &str>(0).to_uppercase(),
            Filas {
                generacion: match row.get(1) {
                    0 => String::from("N"),
                    _ => roman::to(row.get(1)).unwrap(),
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
                                    if let Some((clave, datos)) =
                                        map.get_key_value(&cap[0].to_uppercase())
                                    {
                                        texto.push_str(&format!(
                                            "{}  {} {}, gen {}\n",
                                            clave,
                                            datos.nombre,
                                            datos
                                                .apellidos
                                                .split_whitespace()
                                                .next()
                                                .expect("No se pudo separar apellidos"),
                                            datos.generacion,
                                        ));
                                    }
                                }
                            }

                            Comando::NombreAzul => {
                                for palabra in data.split_whitespace() {
                                    if palabra.len() > 2 {
                                        let palabra = &deunicode(palabra).to_lowercase();

                                        texto.push_str(
                                            &map.par_iter()
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

                            Comando::ApellidoAzul => {
                                for palabra in data.split_whitespace() {
                                    if palabra.len() > 2 {
                                        let palabra = &deunicode(palabra).to_lowercase();

                                        texto.push_str(
                                            &map.par_iter()
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

                            Comando::ClaveInterno => {
                                texto.push_str(GEN_ACTUAL);
                                for cap in re_internos.captures_iter(data) {
                                    if let Some((clave, datos)) =
                                        map.get_key_value(&cap[0].to_uppercase())
                                    {
                                        texto.push_str(&format!(
                                            "{}  {} {}\n",
                                            clave,
                                            datos.nombre,
                                            datos
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

                                        texto.push_str(
                                            &map.par_iter()
                                                .filter_map(|(clave, datos)| {
                                                    if !clave.ends_with(char::is_numeric)
                                                        && deunicode(&datos.nombre)
                                                            .to_lowercase()
                                                            .contains(palabra)
                                                    {
                                                        Some(format!(
                                                            "{}  {} {}\n",
                                                            clave,
                                                            datos.nombre,
                                                            datos
                                                                .apellidos
                                                                .split_whitespace()
                                                                .next()
                                                                .expect(
                                                                    "Error al separar apellidos"
                                                                ),
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
                                                    &map.par_iter()
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
                                                    "No tengo datos sobre esa generación...\n\n",
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
                thread::sleep(Duration::from_millis(500));
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

struct Filas {
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
    Generacion,
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
                r"/[gG]",
                r"/[hH]",
                r"/[sS]",
            ])
            .expect("Error al compilar Regex");
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
            Self::Generacion
        } else if matches.matched(6) {
            Self::Ayuda
        } else if matches.matched(7) {
            Self::Start
        } else {
            Self::None
        }
    }
}

static GEN_ACTUAL: &str = "Gen XXXIII\n\n";

static START: &str = r#"Hola soy el SistemedicBot.
Para buscar...
-Clave usa /clave más las claves. 
-Internos por nombre usa /inombre más los nombres a buscar.
-Internos por clave usa /iclave más las claves.
-Generación entera /generacion más la generación.
-Nombre usa /nombre más los nombres.
-Apellido usa /apellido más los apellidos.

Ejemplo
/clave A101 A027 A007 A010* A010
/nombre Luis
/generacion 33
/iclave cKGr
/inombre Karol

Para ayuda usa /help
Comparte con https://t.me/sistemedicbot"#;

static AYUDA: &str = r#"Para buscar por clave usa /clave ó /c más las claves a buscar.
Para buscar por nombre usa /nombre ó /n más los nombres a buscar.
Para buscar por apellido usa /apellido ó /a más los apellidos a buscar.
Para buscar internos por clave usa /iclave ó /ic más las claves.
Para buscar internos por nombre usa /inombre ó /in más los nombres a buscar.
Para buscar generaciones enteras usa /generacion ó /g más las generaciones. 

Ejemplo
/g 32 33 19
/clave A101 A007 A010*
/c a342
/iclave cKGr
/in Sam
/a castillo

Código Fuente: https://github.com/mucinoab/SistemedicBotRust/"#;

static NO_ENTIENDO: &str = "No te entendí...\nIntenta de nuevo o usa \"/h\" para ayuda.";

static NO_ENCONTRE: &str = "No encontré lo que buscas...\nIntenta de nuevo.";
