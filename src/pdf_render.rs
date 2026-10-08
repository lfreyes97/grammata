//! Rasterizado de páginas PDF con `hayro` en un hilo aparte.
//!
//! La UI publica cada frame la lista de páginas que necesita (`set_jobs`):
//! la cola se *reemplaza*, así que lo que deja de verse al hacer scroll no se
//! llega a renderizar. El hilo recuerda qué (página, escala) ya entregó en la
//! generación actual para no repetir trabajo; `forget` lo invalida cuando la
//! UI libera una textura.

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::{RenderCache, RenderSettings};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

/// Lado máximo de una página rasterizada (px).
pub const MAX_SIDE: f32 = 6000.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Job {
    pub page: usize,
    /// Píxeles por punto PDF.
    pub scale: f32,
}

pub enum Event {
    /// PDF cargado: tamaño de cada página en puntos.
    Loaded { generation: u64, sizes: Vec<egui::Vec2> },
    Failed { generation: u64, error: String },
    Page { generation: u64, job: Job, image: egui::ColorImage },
}

#[derive(Default)]
struct Shared {
    pdf: Option<(u64, Arc<Vec<u8>>)>,
    jobs: Vec<Job>,
    jobs_generation: u64,
    forget: Vec<usize>,
    quit: bool,
}

type SharedPair = Arc<(Mutex<Shared>, Condvar)>;

pub struct PdfRenderer {
    shared: SharedPair,
    rx: Receiver<Event>,
    next_generation: u64,
    last_jobs: Vec<Job>,
}

impl PdfRenderer {
    pub fn new(ctx: egui::Context) -> Self {
        let shared: SharedPair = Arc::default();
        let (tx, rx) = channel();
        let s = shared.clone();
        std::thread::Builder::new()
            .name("pdf-render".into())
            .spawn(move || worker(s, tx, ctx))
            .expect("hilo pdf-render");
        Self { shared, rx, next_generation: 0, last_jobs: Vec::new() }
    }

    /// Carga un PDF nuevo; devuelve su generación.
    pub fn load(&mut self, data: Arc<Vec<u8>>) -> u64 {
        self.next_generation += 1;
        let generation = self.next_generation;
        let (m, cv) = &*self.shared;
        let mut s = m.lock().unwrap();
        s.pdf = Some((generation, data));
        s.jobs.clear();
        s.forget.clear();
        self.last_jobs.clear();
        cv.notify_one();
        generation
    }

    pub fn set_jobs(&mut self, generation: u64, jobs: Vec<Job>) {
        if jobs == self.last_jobs {
            return;
        }
        let (m, cv) = &*self.shared;
        let mut s = m.lock().unwrap();
        s.jobs = jobs.clone();
        s.jobs_generation = generation;
        self.last_jobs = jobs;
        cv.notify_one();
    }

    pub fn forget(&mut self, page: usize) {
        let (m, _) = &*self.shared;
        m.lock().unwrap().forget.push(page);
        self.last_jobs.clear();
    }

    pub fn try_recv(&self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}

impl Drop for PdfRenderer {
    fn drop(&mut self) {
        let (m, cv) = &*self.shared;
        if let Ok(mut s) = m.lock() {
            s.quit = true;
            cv.notify_one();
        }
    }
}

fn worker(shared: SharedPair, tx: Sender<Event>, ctx: egui::Context) {
    let (m, cv) = &*shared;
    loop {
        // Espera un PDF.
        let (generation, data) = {
            let mut s = m.lock().unwrap();
            loop {
                if s.quit {
                    return;
                }
                if let Some(p) = s.pdf.take() {
                    break p;
                }
                s = cv.wait(s).unwrap();
            }
        };
        let pdf = match Pdf::new(data) {
            Ok(p) => p,
            Err(e) => {
                let _ = tx.send(Event::Failed { generation, error: format!("PDF ilegible: {e:?}") });
                ctx.request_repaint();
                continue;
            }
        };
        let pages = pdf.pages();
        let sizes = pages
            .iter()
            .map(|p| {
                let (w, h) = p.render_dimensions();
                egui::vec2(w, h)
            })
            .collect();
        if tx.send(Event::Loaded { generation, sizes }).is_err() {
            return;
        }
        ctx.request_repaint();

        let cache = RenderCache::new();
        let settings = InterpreterSettings::default();
        // Escala (en bits) entregada por página en esta generación.
        let mut done: HashMap<usize, u32> = HashMap::new();
        loop {
            let job = {
                let mut s = m.lock().unwrap();
                loop {
                    if s.quit {
                        return;
                    }
                    if s.pdf.is_some() {
                        break None; // llegó otro PDF: reinicia
                    }
                    for p in s.forget.drain(..) {
                        done.remove(&p);
                    }
                    if s.jobs_generation == generation {
                        let next = s
                            .jobs
                            .iter()
                            .position(|j| done.get(&j.page) != Some(&j.scale.to_bits()));
                        if let Some(i) = next {
                            break Some(s.jobs.remove(i));
                        }
                    }
                    s = cv.wait(s).unwrap();
                }
            };
            let Some(job) = job else { break };
            let Some(page) = pages.get(job.page) else { continue };
            done.insert(job.page, job.scale.to_bits());
            let pix = hayro::render(
                page,
                &cache,
                &settings,
                &RenderSettings {
                    x_scale: job.scale,
                    y_scale: job.scale,
                    bg_color: WHITE,
                    ..Default::default()
                },
            );
            let size = [pix.width() as usize, pix.height() as usize];
            let image = egui::ColorImage::from_rgba_premultiplied(size, pix.data_as_u8_slice());
            if tx.send(Event::Page { generation, job, image }).is_err() {
                return;
            }
            ctx.request_repaint();
        }
    }
}
