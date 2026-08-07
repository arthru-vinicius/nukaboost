//! Leases concedidas a agentes automatizados (seção 12 do plano).
//!
//! Uma lease representa a necessidade de um processo externo (um build, um
//! agente Codex/Claude) de manter o NukaBoost ativo por um período limitado.
//! Diferente da configuração e do journal de recuperação, leases **não são
//! persistidas em disco** — a seção 12 é explícita: "Leases não são
//! restauradas depois de crash/reboot". Elas vivem apenas na memória do
//! processo principal, e usamos [`Instant`] (relógio monotônico) em vez de
//! um timestamp de parede para calcular expiração, o que as torna imunes a
//! ajustes manuais do relógio do sistema.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::error::{NukaError, NukaResult};

/// Limite defensivo para entradas IPC. Leases mais longas devem ser
/// renovadas explicitamente, evitando que um cliente defeituoso mantenha o
/// computador ativo por tempo praticamente ilimitado.
pub const MAX_LEASE_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Uma concessão temporária de "manter ativo", identificada por UUID.
#[derive(Debug, Clone)]
pub struct Lease {
    /// Identificador único, devolvido ao chamador de [`LeaseManager::acquire`]
    /// e exigido por [`LeaseManager::release`].
    pub id: Uuid,
    /// Motivo legível, por exemplo `"cargo build --release"`.
    pub reason: String,
    /// Identificador do dono da lease, por exemplo `"codex"` ou `"claude"`.
    pub owner: String,
    /// Instante em que a lease foi concedida.
    pub acquired_at: Instant,
    /// Tempo de vida máximo da lease. Toda lease deve ter um TTL — não há
    /// construtor que permita omiti-lo, conforme a regra da seção 12.
    pub ttl: Duration,
}

impl Lease {
    /// Instante em que esta lease expira.
    pub fn expires_at(&self) -> Instant {
        self.acquired_at + self.ttl
    }

    /// Verdadeiro se a lease já expirou no instante `now`.
    pub fn is_expired(&self, now: Instant) -> bool {
        now >= self.expires_at()
    }

    /// Tempo restante até a expiração, saturando em zero.
    pub fn remaining(&self, now: Instant) -> Duration {
        self.expires_at().saturating_duration_since(now)
    }
}

/// Registro em memória de todas as leases ativas do processo principal.
#[derive(Debug, Default)]
pub struct LeaseManager {
    leases: HashMap<Uuid, Lease>,
}

impl LeaseManager {
    /// Cria um registro de leases vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Concede uma nova lease com o motivo, dono e TTL informados, e
    /// retorna seu identificador. Leases expiradas são descartadas antes de
    /// registrar a nova, para que o registro não cresça indefinidamente.
    pub fn acquire(
        &mut self,
        reason: impl Into<String>,
        owner: impl Into<String>,
        ttl: Duration,
    ) -> NukaResult<Uuid> {
        self.reap_expired();
        let reason = reason.into().trim().to_owned();
        let owner = owner.into().trim().to_owned();
        if reason.is_empty() || owner.is_empty() {
            return Err(NukaError::Other(
                "lease reason and owner must not be empty".into(),
            ));
        }
        if ttl.is_zero() || ttl > MAX_LEASE_TTL {
            return Err(NukaError::Other(format!(
                "lease TTL must be between 1 second and {} seconds",
                MAX_LEASE_TTL.as_secs()
            )));
        }
        let id = Uuid::new_v4();
        self.leases.insert(
            id,
            Lease {
                id,
                reason,
                owner,
                acquired_at: Instant::now(),
                ttl,
            },
        );
        Ok(id)
    }

    /// Libera exatamente a lease identificada por `id`. Não afeta nenhuma
    /// outra lease — a seção 12 exige que `release` afete somente a lease
    /// informada.
    pub fn release(&mut self, id: Uuid) -> NukaResult<()> {
        self.leases
            .remove(&id)
            .map(|_| ())
            .ok_or_else(|| NukaError::LeaseNotFound(id.to_string()))
    }

    /// Libera todas as leases de uma vez. Reservado para o clique manual em
    /// `Stop`/`Parar` (prioridade humana) e para `stop --force`, nunca para
    /// uso automático por agentes.
    pub fn release_all(&mut self) -> usize {
        let count = self.leases.len();
        self.leases.clear();
        count
    }

    /// Remove leases cujo TTL já expirou e retorna quantas foram removidas.
    pub fn reap_expired(&mut self) -> usize {
        let now = Instant::now();
        let before = self.leases.len();
        self.leases.retain(|_, lease| !lease.is_expired(now));
        before - self.leases.len()
    }

    /// Número de leases ainda válidas, após descartar as expiradas.
    pub fn active_count(&mut self) -> usize {
        self.reap_expired();
        self.leases.len()
    }

    /// Verdadeiro se `id` corresponde a uma lease atualmente válida.
    pub fn contains(&mut self, id: Uuid) -> bool {
        self.reap_expired();
        self.leases.contains_key(&id)
    }

    /// Cópia de todas as leases ainda válidas, para relatar em
    /// `nukaboostctl status --json`.
    pub fn snapshot(&mut self) -> Vec<Lease> {
        self.reap_expired();
        self.leases.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn acquire_then_release_removes_lease() {
        let mut manager = LeaseManager::new();
        let id = manager
            .acquire("cargo build", "codex", Duration::from_secs(60))
            .unwrap();
        assert_eq!(manager.active_count(), 1);

        manager.release(id).unwrap();
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn releasing_unknown_lease_errors() {
        let mut manager = LeaseManager::new();
        let bogus = Uuid::new_v4();
        assert!(manager.release(bogus).is_err());
    }

    #[test]
    fn release_only_affects_target_lease() {
        let mut manager = LeaseManager::new();
        let a = manager
            .acquire("task a", "codex", Duration::from_secs(60))
            .unwrap();
        let b = manager
            .acquire("task b", "claude", Duration::from_secs(60))
            .unwrap();

        manager.release(a).unwrap();

        assert_eq!(manager.active_count(), 1);
        assert!(manager.contains(b));
    }

    #[test]
    fn expired_leases_are_reaped() {
        let mut manager = LeaseManager::new();
        manager
            .acquire("short task", "codex", Duration::from_millis(10))
            .unwrap();
        sleep(Duration::from_millis(30));

        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn release_all_clears_everything() {
        let mut manager = LeaseManager::new();
        manager
            .acquire("a", "codex", Duration::from_secs(60))
            .unwrap();
        manager
            .acquire("b", "claude", Duration::from_secs(60))
            .unwrap();

        let released = manager.release_all();

        assert_eq!(released, 2);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn rejects_invalid_lease_input() {
        let mut manager = LeaseManager::new();
        assert!(manager
            .acquire("", "codex", Duration::from_secs(1))
            .is_err());
        assert!(manager
            .acquire("build", "", Duration::from_secs(1))
            .is_err());
        assert!(manager.acquire("build", "codex", Duration::ZERO).is_err());
        assert!(manager
            .acquire("build", "codex", MAX_LEASE_TTL + Duration::from_secs(1))
            .is_err());
    }
}
