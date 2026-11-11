package cc.straysmp.capes.menu;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Properties;

import cc.straysmp.capes.StrayCapes;
import net.fabricmc.loader.api.FabricLoader;

/**
 * Where the player dragged the PvP Plus HUD groups (offsets in GUI pixels from their normal
 * place), stored in {@code config/strayclient/hud.properties}. Keystrokes keeps its own position.
 */
public final class HudOffsets {
    public static int infoDx, infoDy, armorDx, armorDy;
    private static boolean loaded;

    private HudOffsets() {}

    private static Path file() {
        return FabricLoader.getInstance().getConfigDir().resolve("strayclient").resolve("hud.properties");
    }

    public static synchronized void load() {
        if (loaded) return;
        loaded = true;
        Properties p = new Properties();
        try (InputStream in = Files.newInputStream(file())) {
            p.load(in);
        } catch (IOException ignored) {
            return;
        }
        infoDx = num(p, "infoDx");
        infoDy = num(p, "infoDy");
        armorDx = num(p, "armorDx");
        armorDy = num(p, "armorDy");
    }

    private static int num(Properties p, String key) {
        try {
            return Integer.parseInt(p.getProperty(key, "0").trim());
        } catch (NumberFormatException e) {
            return 0;
        }
    }

    public static synchronized void save() {
        Properties p = new Properties();
        p.setProperty("infoDx", Integer.toString(infoDx));
        p.setProperty("infoDy", Integer.toString(infoDy));
        p.setProperty("armorDx", Integer.toString(armorDx));
        p.setProperty("armorDy", Integer.toString(armorDy));
        try {
            Files.createDirectories(file().getParent());
            try (OutputStream out = Files.newOutputStream(file())) {
                p.store(out, "HUD positions (Stray Client menu -> Move HUD)");
            }
        } catch (IOException e) {
            StrayCapes.LOG.warn("Could not save {}", file(), e);
        }
    }
}
