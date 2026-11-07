package dev.vjeux.mctas.actor;

import net.fabricmc.api.ClientModInitializer;

/** Fabric entry point of the actor (the scripted survival player; see {@link Actor}). */
public final class ActorMod implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        Actor.start();
    }
}
