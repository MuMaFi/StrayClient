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
 * Changes made in the in-game menu, for the Stray Client launcher: it reads
 * {@code config/strayclient/ingame.properties} into its own settings (so the next "Play"
 * keeps them) and deletes the file. Keys: {@code keystrokes}, {@code cape},
 * {@code mod.<file name>} = on/off.
 */
final class Pending {
    private Pending() {}

    static Path file() {
        return FabricLoader.getInstance().getConfigDir().resolve("strayclient").resolve("ingame.properties");
    }

    static synchronized Properties load() {
        Properties p = new Properties();
        try (InputStream in = Files.newInputStream(file())) {
            p.load(in);
        } catch (IOException ignored) {
        }
        return p;
    }

    static synchronized void put(String key, String value) {
        Properties p = load();
        p.setProperty(key, value);
        try {
            Files.createDirectories(file().getParent());
            try (OutputStream out = Files.newOutputStream(file())) {
                p.store(out, "Changes from the in-game menu (read by the Stray Client launcher)");
            }
        } catch (IOException e) {
            StrayCapes.LOG.warn("Could not save {}", file(), e);
        }
    }

    static synchronized void remove(String key) {
        Properties p = load();
        if (p.remove(key) == null) return;
        try (OutputStream out = Files.newOutputStream(file())) {
            p.store(out, "Changes from the in-game menu (read by the Stray Client launcher)");
        } catch (IOException e) {
            StrayCapes.LOG.warn("Could not save {}", file(), e);
        }
    }
}
