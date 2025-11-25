use contract::Actor;

#[derive(Debug, Clone)]
pub struct CleanCacheActorPayload {
    pub force: bool,
}

pub struct CleanCacheActor {
    payload: CleanCacheActorPayload,
}

impl Actor<CleanCacheActorPayload> for CleanCacheActor {
    fn with(payload: CleanCacheActorPayload) -> Self {
        Self { payload }
    }

    async fn run(&self) -> contract::Result<()> {
        if !self.payload.force {
            println!("⚠️  Cache cleanup requires confirmation.");
            println!("If you're sure what you're doing - add --force flag");
            return Ok(());
        }

        let cache_dir = contract::get_package_cache_dir();

        if !cache_dir.exists() {
            debug::info!("Cache directory does not exist: {:?}", cache_dir);
            return Ok(());
        }

        println!("Hope you're sure what you're doing");

        tokio::fs::remove_dir_all(&cache_dir).await?;
        tokio::fs::create_dir_all(&cache_dir).await?;
        println!("✓ Cleaned all package cache");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_cache_actor_without_force() {
        let payload = CleanCacheActorPayload { force: false };

        let actor = CleanCacheActor::with(payload);
        assert!(!actor.payload.force);
    }
}
