package cc.straysmp.capes.menu;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Properties;

import cc.straysmp.capes.StrayCapes;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.Minecraft;

/**
 * Counts the time spent in a world (singleplayer or server) for the launcher's profile cards:
 * {@code config/strayclient/playtime.properties}, key {@code seconds}. Saved every minute
 * and when leaving a world.
 */
public final class Playtime {
    private static long seconds = -1;
    private static int ticks;
    private static int unsaved;
    private static boolean inWorld;

    private Playtime() {}

    private static Path file() {
        return FabricLoader.getInstance().getConfigDir().resolve("strayclient").resolve("playtime.properties");
    }

    private static void load() {
        seconds = 0;
        Properties p = new Properties();
        try (InputStream in = Files.newInputStream(file())) {
            p.load(in);
            seconds = Long.parseLong(p.getProperty("seconds", "0").trim());
        } catch (IOException | NumberFormatException ignored) {
        }
    }

    private static void save() {
        unsaved = 0;
        Properties p = new Properties();
        p.setProperty("seconds", Long.toString(seconds));
        try {
            Files.createDirectories(file().getParent());
            try (OutputStream out = Files.newOutputStream(file())) {
                p.store(out, "Time played (Stray Client)");
            }
        } catch (IOException e) {
            StrayCapes.LOG.warn("Could not save {}", file(), e);
        }
    }

    /** Client tick (20 per second). */
    public static void tick(Minecraft mc) {
        boolean now = mc.player != null && mc.level != null;
        if (!now) {
            if (inWorld && unsaved > 0) save(); // just left a world
            inWorld = false;
            return;
        }
        inWorld = true;
        if (seconds < 0) load();
        if (++ticks < 20) return;
        ticks = 0;
        seconds++;
        if (++unsaved >= 60) save();
    }
}
