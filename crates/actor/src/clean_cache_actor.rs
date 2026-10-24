use contract::{Actor, Context};

#[derive(Debug, Clone)]
pub struct CleanCacheActorPayload {
    pub force: bool,
}

pub struct CleanCacheActor {
    context: Context,
    payload: CleanCacheActorPayload,
}

impl Actor<CleanCacheActorPayload> for CleanCacheActor {
    fn with(context: Context, payload: CleanCacheActorPayload) -> Self {
        Self { context, payload }
    }

    async fn run(&self) -> contract::Result<()> {
        if !self.payload.force {
            println!("⚠️  Cache cleanup requires confirmation.");
            println!("If you're sure what you're doing - add --force flag");
            return Ok(());
        }

        let store_dir = self.context.store_dir();
        let legacy_cache_dir = self.context.legacy_cache_dir();
        let has_cache = store_dir.exists() || legacy_cache_dir.exists();
        if !has_cache {
            debug::info!("Store directory does not exist: {:?}", store_dir);
            return Ok(());
        }

        println!("Hope you're sure what you're doing");

        fsutil::remove_entry(&store_dir).await?;
        fsutil::remove_entry(&legacy_cache_dir).await?;
        tokio::fs::create_dir_all(&store_dir).await?;
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

        let context = Context::new("/work/app".into(), "/home/me/.craft".into(), false);
        let actor = CleanCacheActor::with(context, payload);
        assert!(!actor.payload.force);
    }
}
