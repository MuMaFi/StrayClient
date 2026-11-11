package cc.straysmp.capes.menu;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Properties;

import cc.straysmp.capes.StrayCapes;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.server.IntegratedServer;
import net.minecraft.network.chat.Component;
import net.minecraft.server.MinecraftServer;
import net.minecraft.util.HttpUtil;

/**
 * "Host": share a singleplayer world with friends anywhere. The launcher writes
 * {@code config/strayclient/host.properties} (world folder + session name); on the title screen
 * that world is opened and published. Publishing is a normal "Open to LAN"; the e4mc mod
 * (installed by the launcher) then gives it a public address and prints it in chat.
 */
public final class Hosting {
    private static String world;
    private static String name = "";
    private static boolean opening;
    private static int waitTicks;

    private Hosting() {}

    /** Read (and remove) a pending request from the launcher. Called once at start-up. */
    public static void load() {
        Path file = FabricLoader.getInstance().getConfigDir().resolve("strayclient").resolve("host.properties");
        Properties p = new Properties();
        try (InputStream in = Files.newInputStream(file)) {
            p.load(in);
        } catch (IOException e) {
            return;
        }
        try {
            Files.deleteIfExists(file); // one start only: the next normal launch must not host
        } catch (IOException ignored) {
        }
        String w = p.getProperty("world", "").trim();
        if (w.isEmpty() || w.contains("/") || w.contains("\\") || w.contains("..")) return;
        if (!Files.isDirectory(FabricLoader.getInstance().getGameDir().resolve("saves").resolve(w))) return;
        world = w;
        name = p.getProperty("name", "").trim();
        StrayCapes.LOG.info("Host request for world {}", w);
    }

    /** Client tick: open the requested world from the title screen, then publish it. */
    public static void tick(Minecraft mc) {
        if (world == null) return;
        if (!opening) {
            if (mc.gui.overlay() == null && mc.gui.screen() instanceof TitleScreen) {
                opening = true;
                String w = world;
                mc.createWorldOpenFlows().openWorld(w, () -> {
                    world = null; // cancelled (e.g. a backup question was declined)
                    mc.gui.setScreen(new TitleScreen());
                });
            }
            return;
        }
        if (mc.player == null || !mc.hasSingleplayerServer()) return;
        if (++waitTicks < 40) return; // let the world settle for two seconds
        world = null;
        publish(mc, name);
    }

    public static boolean canHost(Minecraft mc) {
        IntegratedServer server = mc.getSingleplayerServer();
        return mc.player != null && server != null && !server.isPublished();
    }

    /** Publish the running singleplayer world; e4mc turns that into a public address. */
    public static void publish(Minecraft mc, String sessionName) {
        IntegratedServer server = mc.getSingleplayerServer();
        if (server == null) return;
        try {
            if (!server.isPublished() && !server.publishServer(MinecraftServer.MultiplayerScope.LAN, HttpUtil.getAvailablePort())) {
                say(mc, Component.translatable("strayclient.host.failed").withStyle(ChatFormatting.RED));
                return;
            }
            if (sessionName != null && !sessionName.isEmpty()) server.setMotd(sessionName);
        } catch (RuntimeException e) {
            StrayCapes.LOG.warn("Hosting failed", e);
            say(mc, Component.translatable("strayclient.host.failed").withStyle(ChatFormatting.RED));
            return;
        }
        boolean global = FabricLoader.getInstance().isModLoaded("e4mc");
        say(mc, Component.literal("Stray Client » ").withColor(0x51A2FF)
                .append(Component.translatable(global ? "strayclient.host.on" : "strayclient.host.lan").withStyle(ChatFormatting.WHITE)));
    }

    private static void say(Minecraft mc, Component message) {
        mc.gui.hud.getChat().addClientSystemMessage(message);
    }
}
