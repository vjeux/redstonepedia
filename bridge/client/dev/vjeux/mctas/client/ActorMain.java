package dev.vjeux.mctas.client;

import dev.vjeux.mctas.actor.Actor;

/**
 * Launches the real Minecraft client unchanged (HUD included: the actor plays survival in first
 * person), plus the actor's command thread ({@link Actor}: the scripted player of `mctas bedrock
 * video`; the Fabric mod `bridge/actor` is the same driver under Fabric Loader).
 */
public final class ActorMain {
    private ActorMain() {}

    public static void main(String[] args) throws Exception {
        Actor.start();
        net.minecraft.client.main.Main.main(args);
    }
}
