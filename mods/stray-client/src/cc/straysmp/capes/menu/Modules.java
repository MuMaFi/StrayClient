package cc.straysmp.capes.menu;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Properties;

import cc.straysmp.capes.StrayCapes;
import net.fabricmc.loader.api.FabricLoader;
import net.minecraft.client.gui.screens.Screen;

/**
 * Live module toggles. Keystrokes and PvP Plus are separate mods, so they are reached through
 * reflection and only offered when they are loaded.
 */
final class Modules {
    private static final String KS_CONFIG = "net.marblock.keystrokes.config.KeystrokesConfig";
    private static final String PVP_CONFIG = "de.pvpplus.Config";
    private static final String PVP_SCREEN = "de.pvpplus.PvpScreen";

    private Modules() {}

    // ── Keystrokes ──
    static boolean keystrokesLoaded() {
        return FabricLoader.getInstance().isModLoaded("modern-keystrokes") && cls(KS_CONFIG) != null;
    }

    static boolean keystrokes() {
        try {
            return cls(KS_CONFIG).getField("isEnabled").getBoolean(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return false;
        }
    }

    static void setKeystrokes(boolean on) {
        try {
            Class<?> c = cls(KS_CONFIG);
            c.getField("isEnabled").setBoolean(null, on);
            c.getMethod("save").invoke(null);
            Pending.put("keystrokes", Boolean.toString(on));
        } catch (ReflectiveOperationException | RuntimeException e) {
            StrayCapes.LOG.warn("Keystrokes toggle failed", e);
        }
    }

    // ── Cape (our own mod) ──
    private static Path capeConfig() {
        return FabricLoader.getInstance().getConfigDir().resolve("stray-capes").resolve("config.properties");
    }

    private static Properties capeProps() {
        Properties p = new Properties();
        try (InputStream in = Files.newInputStream(capeConfig())) {
            p.load(in);
        } catch (IOException ignored) {
        }
        return p;
    }

    static boolean capeOn() {
        return "local".equals(capeProps().getProperty("mode", "account").trim());
    }

    static boolean hasCapeFile() {
        return !capeProps().getProperty("cape", "").trim().isEmpty();
    }

    static void setCape(boolean on) {
        Properties p = capeProps();
        String mode = on ? "local" : "account";
        p.setProperty("mode", mode);
        try {
            Files.createDirectories(capeConfig().getParent());
            try (OutputStream out = Files.newOutputStream(capeConfig())) {
                p.store(out, "Stray Capes (managed by Stray Client)");
            }
            Pending.put("cape", mode);
        } catch (IOException e) {
            StrayCapes.LOG.warn("Could not save cape config", e);
        }
    }

    // ── PvP Plus ──
    static boolean pvpLoaded() {
        return FabricLoader.getInstance().isModLoaded("pvpplus") && cls(PVP_CONFIG) != null;
    }

    private static Object pvpConfig() throws ReflectiveOperationException {
        return cls(PVP_CONFIG).getMethod("get").invoke(null);
    }

    static boolean pvp(String field) {
        try {
            Object cfg = pvpConfig();
            return cfg.getClass().getField(field).getBoolean(cfg);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return false;
        }
    }

    static void setPvp(String field, boolean on) {
        try {
            Object cfg = pvpConfig();
            Field f = cfg.getClass().getField(field);
            f.setBoolean(cfg, on);
            cls(PVP_CONFIG).getMethod("save").invoke(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            StrayCapes.LOG.warn("PvP Plus toggle {} failed", field, e);
        }
    }

    /** PvP Plus' own settings screen (all of its options), or null. */
    static Screen pvpScreen(Screen parent) {
        try {
            Class<?> c = cls(PVP_SCREEN);
            return c == null ? null : (Screen) c.getConstructor(Screen.class).newInstance(parent);
        } catch (ReflectiveOperationException | RuntimeException e) {
            return null;
        }
    }

    private static Class<?> cls(String name) {
        try {
            return Class.forName(name, true, Modules.class.getClassLoader());
        } catch (ClassNotFoundException | LinkageError e) {
            return null;
        }
    }
}
