//! F3 — adaptadores concretos del bucle agéntico.
//!
//! Conectan el núcleo (`agent_loop.rs`) con lo que ya existe en el proyecto
//! **sin tocar los módulos de otros agentes**:
//!
//! * `ProvidersBackend`: habla con `providers::execute_chat` (W6) como
//!   backend de chat.  Si la llamada **cae y se usa el fallback local**, se
//!   reporta como **fallo del proveedor** para que el circuit breaker aprenda
//!   (y no se repita el mismo proveedor).
//! * `ServiceBusRouter`: enruta la petición al nodo/servicio correcto
//!   usando el `service_bus` (W6): `ServiceBus::select(TaskType)`.
//!
//! El núcleo no depende de estos adaptadores: son implementaciones de los
//! traits `ChatBackend` y `Router` ⇒ se pueden sustituir en tests.

use crate::agent_loop::{ChatBackend, ChatOutcome, ProviderRef, Router};
use crate::providers;
use crate::service_bus::{ServiceBus, TaskType};

/// Backend de chat sobre `providers::execute_chat`.
pub struct ProvidersBackend {
    /// Tope de tokens de la respuesta.
    pub max_tokens: u32,
    /// Prompt de sistema (opcional).  **Nunca** debe contener credenciales.
    pub system: Option<String>,
}

impl Default for ProvidersBackend {
    fn default() -> Self {
        ProvidersBackend {
            max_tokens: 1024,
            system: None,
        }
    }
}

impl ChatBackend for ProvidersBackend {
    fn chat(&mut self, provider: &ProviderRef, prompt: &str) -> Result<ChatOutcome, String> {
        let r = providers::execute_chat(
            prompt,
            Some(provider.model.as_str()),
            None,
            self.system.as_deref(),
            self.max_tokens,
        );
        // `execute_chat` no devuelve error: cuando el proveedor cae usa el
        // fallback local y lo marca con `fallback_used`.  Para el bucle eso
        // ES un fallo del proveedor (asi el breaker lo penaliza y la cadena
        // pasa al siguiente en vez de quedarse creyendo que todo va bien).
        if r.fallback_used {
            return Err(format!(
                "el proveedor {} cayo y se uso el fallback local",
                provider.id
            ));
        }
        if r.reply.trim().is_empty() {
            return Err(format!(
                "el proveedor {} devuelve una respuesta vacia",
                provider.id
            ));
        }
        Ok(ChatOutcome {
            reply: r.reply,
            // `execute_chat` no expone los tokens del prompt: se contabilizan
            // 0 (=> el coste sale del output; el `PriceBook` lo documenta).
            tokens_used: r.tokens_used,
            prompt_tokens: 0,
            latency_ms: r.latency_ms,
            model_id: r.model_id,
        })
    }
}

/// Router del meta-agente sobre el `service_bus`.
pub struct ServiceBusRouter {
    pub bus: ServiceBus,
}

impl ServiceBusRouter {
    pub fn new(bus: ServiceBus) -> Self {
        ServiceBusRouter { bus }
    }
}

/// Traduce el nombre libre de tipo de tarea al `TaskType` del bus.
pub fn task_type_of(name: &str) -> TaskType {
    match name.trim().to_lowercase().as_str() {
        "build" | "code" | "codigo" => TaskType::Build,
        "analysis" | "analisis" | "re" => TaskType::Analysis,
        "writing" | "texto" | "redaccion" => TaskType::Writing,
        _ => TaskType::Inference,
    }
}

impl Router for ServiceBusRouter {
    /// Enruta al servicio que el bus elige para ese tipo de tarea.  Si el bus
    /// no tiene ninguno, se devuelve la **cadena** de fallback (para que el
    /// bucle pueda probar el siguiente) ⇒ si no hay nada, `None`.
    fn route(&self, _provider: &str, task_type: &str) -> Option<String> {
        let sel = self.bus.select(task_type_of(task_type));
        sel.chosen.or_else(|| sel.chain.first().cloned())
    }
}

/// Router de reserva: aplica el criterio fijo "reclamar una tarea del
/// tablero" usando un cierre que el llamante conecta con el tablero real.
pub struct FixedTaskRouter<F>
where
    F: Fn() -> Option<String>,
{
    pub next_free_task: F,
}

impl<F> Router for FixedTaskRouter<F>
where
    F: Fn() -> Option<String>,
{
    fn route(&self, provider: &str, task_type: &str) -> Option<String> {
        Some(format!("{}::{}", provider, task_type))
    }

    fn fixed_fallback_task(&self) -> Option<String> {
        (self.next_free_task)()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_loop::Action;

    #[test]
    fn el_tipo_de_tarea_se_traduce() {
        assert!(matches!(task_type_of("build"), TaskType::Build));
        assert!(matches!(task_type_of("RE"), TaskType::Analysis));
        assert!(matches!(task_type_of("cualquier cosa"), TaskType::Inference));
    }

    #[test]
    fn el_router_de_reserva_aplica_el_criterio_fijo() {
        let r = FixedTaskRouter {
            next_free_task: || Some("t-42".to_string()),
        };
        assert_eq!(r.fixed_fallback_task().as_deref(), Some("t-42"));
        assert!(r.route("p1", "vision").is_some());
        // Sin tarea libre, el criterio fijo es callar (nunca inventar).
        let r2 = FixedTaskRouter {
            next_free_task: || None,
        };
        assert!(r2.fixed_fallback_task().is_none());
    }

    #[test]
    fn el_comando_del_shell_se_saca_de_los_args() {
        let a = Action::Tool {
            name: "shell".to_string(),
            args: "{\"cmd\":\"ls -la\"}".to_string(),
        };
        let cmd = crate::persistent_shell::ShellToolExecutor::command_of(&a);
        assert_eq!(cmd, "ls -la");
    }
}
