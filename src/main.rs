#![feature(once_cell)]

use std::{
    collections::HashMap,
    env,
    fmt::Write,
    lazy::SyncLazy,
    thread,
    time::{Duration, Instant},
};

//TODO
//Todos los internos?

use deunicode::deunicode;
use futures::StreamExt;
use indexmap::IndexMap;
use telegram_bot::{Api, CanSendMessage, MessageKind, UpdateKind};
use tokio_postgres::NoTls;

#[macro_use]
extern crate log;

#[tokio::main]
async fn main() {
    log4rs::init_file("log_config.yml", Default::default()).expect("No se pudo iniciar log");
    info!("Iniciando...");

    let (client, connection) = tokio_postgres::connect(
        &env::var("DATABASE").expect("Variable de base de datos no encontrada"),
        NoTls,
    )
    .await
    .expect("PostgreSQL no encontrada o mal configurada");

    tokio::spawn(async move {
        connection.await.expect("Error al intentar conectar");
    });

    let mut datos: IndexMap<String, Persona> = IndexMap::with_capacity(348);

    client
        .query(
            "SELECT *
                 FROM bot_claves
                 UNION
                 SELECT *
                 FROM bot_internos
                 ORDER BY clave;",
            &[],
        )
        .await
        .expect("Extraer datos de BDD")
        .iter()
        .for_each(|row| {
            let mut clave = String::from(row.get::<usize, &str>(0).trim());
            clave.make_ascii_uppercase();
            datos.insert(
                clave,
                Persona {
                    generacion: row.get::<usize, i32>(1) as i8,
                    nombre: String::from(row.get::<usize, &str>(2).trim()),
                    apellidos: String::from(row.get::<usize, &str>(3).trim()),
                },
            );
        });

    let mut texto = String::new();
    let mut generaciones = Vec::new();
    let mut encontrado;

    let api = Api::new(&env::var("TOKEN").expect("Token no encontrado"));
    let mut stream = api.stream();

    info!("Datos procesados, listo para recibir querys.");

    while let Some(update) = stream.next().await {
        match update {
            Ok(update) => {
                if let UpdateKind::Message(mut message) = update.kind {
                    if let MessageKind::Text { ref mut data, .. } = message.kind {
                        let now = Instant::now();

                        let mut data = deunicode(data);
                        data.make_ascii_uppercase();
                        let mut buscados = Vec::new();
                        let mut palabras = data.split_whitespace();

                        match COMANDOS
                            .get(palabras.next().unwrap_or_default())
                            .unwrap_or(&Comando::None)
                        {
                            Comando::Clave => {
                                for palabra in palabras {
                                    if let Some((clave, persona)) = datos.get_key_value(palabra) {
                                        writeln!(
                                            texto,
                                            "{}  {} {}, gen {}",
                                            clave,
                                            persona.nombre,
                                            persona
                                                .apellidos
                                                .split_whitespace()
                                                .next()
                                                .unwrap_or_default(),
                                            roman(persona.generacion),
                                        )
                                        .unwrap();
                                    }
                                }
                            }

                            Comando::Nombre => {
                                palabras.for_each(|palabra| {
                                    if palabra.len() > 2 {
                                        buscados.push(palabra);
                                    }
                                });

                                for (clave, persona) in &datos {
                                    let mut nombre = deunicode(&persona.nombre);
                                    nombre.make_ascii_uppercase();

                                    encontrado = buscados
                                        .iter()
                                        .any(|nombre_buscado| nombre.contains(nombre_buscado));

                                    if encontrado {
                                        writeln!(texto, "{}  {}", clave, persona.apellidos)
                                            .unwrap();
                                    }
                                }
                            }

                            Comando::Apellido => {
                                palabras.for_each(|palabra| {
                                    if palabra.len() > 2 {
                                        buscados.push(palabra);
                                    }
                                });

                                for (clave, persona) in &datos {
                                    let mut apellidos = deunicode(&persona.apellidos);
                                    apellidos.make_ascii_uppercase();

                                    encontrado = buscados.iter().any(|apellido_buscado| {
                                        apellidos.contains(apellido_buscado)
                                    });

                                    if encontrado {
                                        writeln!(texto, "{}  {}", clave, persona.nombre).unwrap();
                                    }
                                }
                            }

                            Comando::Generacion => {
                                palabras.for_each(|palabra| {

                                    if let Ok(numero) = palabra.parse::<i8>() {
                                                if numero > 15 && numero < 34 {
                                                    generaciones.push(numero);
                                                } else {
                                                    writeln!(
                                                        texto,
                                                        "No tengo datos sobre quien pertenece a la generación {} :(", numero
                                                    )
                                                    .unwrap_or_default();
                                                }
                                        }
                                    });

                                for (clave, datos) in &datos {
                                    if generaciones.iter().any(|n| *n == datos.generacion) {
                                        writeln!(
                                            texto,
                                            "{}  {} {}",
                                            clave,
                                            datos.nombre,
                                            datos
                                                .apellidos
                                                .split_whitespace()
                                                .next()
                                                .unwrap_or_default()
                                        )
                                        .unwrap_or_default();
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

                        if texto.is_empty() {
                            api.spawn(message.chat.text(NO_ENCONTRE));
                        } else {
                            api.spawn(message.chat.text(texto.as_str()));
                        }

                        info!(
                            "{}: {:#?} {:#?}",
                            &message.from.first_name,
                            &data,
                            Instant::now().duration_since(now)
                        );

                        texto.clear();
                        generaciones.clear();
                    }
                }
            }

            Err(e) => {
                error!("{}", e);
                thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

struct Persona {
    nombre: String,
    apellidos: String,
    generacion: i8,
}

#[derive(Clone, Copy)]
enum Comando {
    Clave,
    Nombre,
    Apellido,
    Generacion,
    Ayuda,
    Start,
    None,
}

static COMANDOS: SyncLazy<HashMap<&str, Comando>> = SyncLazy::new(|| {
    [
        ("/C", Comando::Clave),
        ("/N", Comando::Nombre),
        ("/A", Comando::Apellido),
        ("/G", Comando::Generacion),
        ("/H", Comando::Ayuda),
        ("/S", Comando::Start),
        ("/CLAVE", Comando::Clave),
        ("/NOMBRE", Comando::Nombre),
        ("/AYUDA", Comando::Apellido),
        ("/GEN", Comando::Generacion),
        ("/HELP", Comando::Ayuda),
        ("/START", Comando::Start),
    ]
    .iter()
    .map(|(k, v)| (*k, *v))
    .collect()
});

fn roman(mut n: i8) -> String {
    let mut roman = String::new();

    if n == 0 {
        roman.push('N');
    } else {
        for (letra, valor) in &[('X', 10), ('V', 5), ('I', 1)] {
            while n >= *valor {
                n -= valor;
                roman.push(*letra);
            }
        }
    }
    roman
}

static START: &str = r#"Para buscar...
- Clave usa /clave más las claves. 
- Generación entera /gen más la generación.
- Nombre usa /nombre más los nombres.
- Apellido usa /apellido más los apellidos.

Búsquedas incluyen azules e internos. 

Ejemplo
/clave A101 A027 A010* cKGr 
/nombre Luis Karol
/gen 32
/apellido Soriano

Para ayuda usa /help
Comparte con https://t.me/sistemedicbot"#;

static AYUDA: &str = r#"Para buscar por... 
- Clave usa /clave ó /c más las claves.
- Nombre usa /nombre ó /n más los nombres.
- Apellido usa /apellido ó /a más los apellidos.
- Generaciones enteras usa /gen ó /g más las generaciones. 
Búsquedas incluyen azules e internos. 

Ejemplo
/g 32 33
/clave a101 A007 A010*
/c A342 A225 cKGr
/n Sam Pedro
/a castillo

Código Fuente: https://github.com/mucinoab/SistemedicBotRust"#;

static NO_ENTIENDO: &str = "No te entendí...\nIntenta de nuevo o usa \"/h\" para ayuda.";

static NO_ENCONTRE: &str = "No encontré lo que buscas...\nIntenta de nuevo.";
