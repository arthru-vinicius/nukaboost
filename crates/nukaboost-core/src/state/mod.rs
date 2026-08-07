//! Máquina de estados do NukaBoost e cálculo de ativação efetiva.
//!
//! O estado observável de fora (bandeja, `nukaboostctl status`) é sempre um
//! dos cinco valores de [`State`]. A transação de ativação/desativação
//! descrita nas seções 7 e 8 do plano é modelada por [`StateMachine`], que
//! usa um mutex interno para garantir que apenas uma transição esteja em
//! andamento por vez — a mesma exigência do passo 1 da seção 7 ("Obter
//! mutex interno de mudança de estado").

pub mod lease;

use std::fmt;
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::error::{NukaError, NukaResult};

pub use lease::{Lease, LeaseManager};

/// Estado observável do NukaBoost.
///
/// A seção 3 do plano recomenda um terceiro estado (`Error`) além dos dois
/// ícones originalmente pedidos, para nunca exibir informação falsa sobre
/// proteções incompletas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Nenhuma proteção está em vigor; plano de energia original intacto.
    Inactive,
    /// Transição em andamento de `Inactive` para `Active`; ainda não é seguro
    /// reportar sucesso.
    Activating,
    /// Todas as proteções foram aplicadas e confirmadas por leitura.
    Active,
    /// Transição em andamento de volta para `Inactive`.
    Deactivating,
    /// Uma proteção não pôde ser confirmada ou restaurada; requer atenção do
    /// usuário. Nunca deve ser confundido com `Active`.
    Error,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Inactive => "inactive",
            Self::Activating => "activating",
            Self::Active => "active",
            Self::Deactivating => "deactivating",
            Self::Error => "error",
        })
    }
}

impl State {
    /// Lista os estados para os quais é permitido transicionar a partir deste.
    ///
    /// Este é o único lugar do código onde o grafo de transições é
    /// definido; [`StateMachine`] apenas o consulta.
    pub const fn allowed_transitions(self) -> &'static [State] {
        use State::*;
        match self {
            Inactive => &[Activating],
            Activating => &[Active, Error],
            Active => &[Deactivating],
            Deactivating => &[Inactive, Error],
            Error => &[Activating, Deactivating, Inactive],
        }
    }

    /// Verifica se a transição para `target` é permitida a partir deste estado.
    pub fn can_transition_to(self, target: State) -> bool {
        self.allowed_transitions().contains(&target)
    }
}

/// Máquina de estados thread-safe que serializa transições através de um
/// mutex interno, evitando que duas ativações concorrentes corrompam o
/// estado do plano de energia.
pub struct StateMachine {
    /// Estado observável atual. Travado apenas pelo tempo de uma leitura ou
    /// escrita pontual — nunca mantido preso durante uma transação inteira
    /// — para que [`StateMachine::current`] nunca bloqueie por causa de uma
    /// transição em andamento em outra thread.
    current: Mutex<State>,
    /// Serializa transições: quem consegue este mutex tem permissão
    /// exclusiva para levar a máquina de um estado estável a outro. Mantido
    /// travado pelo [`TransitionGuard`] durante toda a transação — este é o
    /// "mutex interno de mudança de estado" do passo 1 da seção 7.
    transition_lock: Mutex<()>,
}

impl Default for StateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMachine {
    /// Cria uma máquina de estados iniciando em [`State::Inactive`], como
    /// exigido pela seção 3 do plano ("Iniciar sempre no estado `Inactive`").
    pub fn new() -> Self {
        Self {
            current: Mutex::new(State::Inactive),
            transition_lock: Mutex::new(()),
        }
    }

    /// Retorna o estado atual. Nunca bloqueia por mais que o tempo de uma
    /// leitura pontual, mesmo que uma transição esteja em andamento em
    /// outra thread.
    pub fn current(&self) -> State {
        *lock(&self.current)
    }

    /// Tenta iniciar uma transição para um estado intermediário
    /// (`Activating` ou `Deactivating`).
    ///
    /// Corresponde ao passo 1 da seção 7 do plano: adquire o mutex interno
    /// de mudança de estado antes de qualquer chamada Win32, e o mantém
    /// travado (via o [`TransitionGuard`] retornado) até que o chamador
    /// finalize a transação com [`TransitionGuard::commit`].
    ///
    /// Se outra transição já estiver em andamento (no mesmo processo, de
    /// qualquer thread — por exemplo um clique na bandeja concorrendo com
    /// um comando IPC), retorna erro imediatamente em vez de bloquear:
    /// preferimos que o chamador veja "ocupado agora" a empilhar chamadas
    /// esperando indefinidamente (mesma filosofia da seção 10: "Não entrar
    /// em loop brigando indefinidamente").
    pub fn begin_transition(&self, intermediate: State) -> NukaResult<TransitionGuard<'_>> {
        let permit = match self.transition_lock.try_lock() {
            Ok(permit) => permit,
            Err(std::sync::TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(NukaError::InvalidTransition {
                    from: format!("{} (transition already in progress)", self.current()),
                    to: intermediate.to_string(),
                });
            }
        };

        let mut current = lock(&self.current);
        if !current.can_transition_to(intermediate) {
            return Err(NukaError::InvalidTransition {
                from: current.to_string(),
                to: intermediate.to_string(),
            });
        }
        *current = intermediate;
        drop(current);

        Ok(TransitionGuard {
            machine: self,
            _permit: permit,
            intermediate,
            committed: false,
        })
    }
}

fn lock(mutex: &Mutex<State>) -> MutexGuard<'_, State> {
    // Um mutex "envenenado" (poisoned) por um panic em outra thread não deve
    // impedir a leitura/gravação do estado: preferimos continuar operando
    // (e possivelmente cair em `Error` por outra via) a travar o processo
    // inteiro por causa de um panic não relacionado.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Alça (RAII) de uma transição de estado em andamento.
///
/// Enquanto o guard existir, nenhuma outra transição pode começar — o
/// `transition_lock` da [`StateMachine`] permanece travado — mas
/// [`StateMachine::current`] continua funcionando normalmente em qualquer
/// thread. Se o guard for descartado sem chamar [`TransitionGuard::commit`]
/// (por exemplo, por um `?` de erro ou um panic), o destrutor grava
/// [`State::Error`] em vez de deixar a máquina presa em um estado
/// intermediário — nunca é aceitável ficar preso em
/// `Activating`/`Deactivating` indefinidamente.
pub struct TransitionGuard<'a> {
    machine: &'a StateMachine,
    _permit: MutexGuard<'a, ()>,
    intermediate: State,
    committed: bool,
}

impl TransitionGuard<'_> {
    /// Estado intermediário em que a máquina se encontra durante a transação.
    pub fn intermediate(&self) -> State {
        self.intermediate
    }

    /// Finaliza a transação, gravando `final_state` como o novo estado
    /// observável e liberando o mutex de transição.
    pub fn commit(mut self, final_state: State) -> NukaResult<()> {
        if !self.intermediate.can_transition_to(final_state) {
            return Err(NukaError::InvalidTransition {
                from: self.intermediate.to_string(),
                to: final_state.to_string(),
            });
        }
        *lock(&self.machine.current) = final_state;
        self.committed = true;
        Ok(())
    }
}

impl Drop for TransitionGuard<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let mut current = lock(&self.machine.current);
            if *current != State::Error {
                *current = State::Error;
            }
        }
        // `_permit` é liberado automaticamente logo após este método
        // retornar, só então permitindo que uma nova transição comece —
        // garantindo que ninguém observe uma janela em que o mutex de
        // transição já esteja livre mas o estado ainda não tenha sido
        // corrigido para `Error`.
    }
}

/// Calcula se o NukaBoost deve estar efetivamente ativo, combinando a
/// retenção manual do usuário com o número de leases concedidas a agentes.
///
/// Reproduz literalmente a fórmula da seção 12 do plano:
/// `manual_hold == true OR active_leases > 0`.
pub const fn is_effectively_active(manual_hold: bool, active_leases: usize) -> bool {
    manual_hold || active_leases > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_inactive() {
        let machine = StateMachine::new();
        assert_eq!(machine.current(), State::Inactive);
    }

    #[test]
    fn full_activation_cycle_succeeds() {
        let machine = StateMachine::new();

        let guard = machine.begin_transition(State::Activating).unwrap();
        assert_eq!(machine.current(), State::Activating);
        guard.commit(State::Active).unwrap();
        assert_eq!(machine.current(), State::Active);

        let guard = machine.begin_transition(State::Deactivating).unwrap();
        assert_eq!(machine.current(), State::Deactivating);
        guard.commit(State::Inactive).unwrap();
        assert_eq!(machine.current(), State::Inactive);
    }

    #[test]
    fn cannot_start_two_transitions_at_once() {
        let machine = StateMachine::new();
        let _guard = machine.begin_transition(State::Activating).unwrap();

        let second = machine.begin_transition(State::Activating);
        assert!(second.is_err());
    }

    #[test]
    fn cannot_stop_from_inactive() {
        let machine = StateMachine::new();
        let result = machine.begin_transition(State::Deactivating);
        assert!(result.is_err());
    }

    #[test]
    fn dropping_guard_without_commit_falls_back_to_error() {
        let machine = StateMachine::new();
        {
            let _guard = machine.begin_transition(State::Activating).unwrap();
            // O guard sai de escopo aqui sem `commit`, simulando um erro
            // não tratado ou um panic no meio da transação.
        }
        assert_eq!(machine.current(), State::Error);
    }

    #[test]
    fn error_state_allows_retry_or_manual_reset() {
        let machine = StateMachine::new();
        {
            let _guard = machine.begin_transition(State::Activating).unwrap();
        }
        assert_eq!(machine.current(), State::Error);

        let guard = machine.begin_transition(State::Activating).unwrap();
        guard.commit(State::Active).unwrap();
        assert_eq!(machine.current(), State::Active);
    }

    #[test]
    fn activation_failure_goes_to_error_not_inactive() {
        let machine = StateMachine::new();
        let guard = machine.begin_transition(State::Activating).unwrap();
        guard.commit(State::Error).unwrap();
        assert_eq!(machine.current(), State::Error);
    }

    #[test]
    fn effective_activation_formula() {
        assert!(!is_effectively_active(false, 0));
        assert!(is_effectively_active(true, 0));
        assert!(is_effectively_active(false, 1));
        assert!(is_effectively_active(true, 3));
    }
}
