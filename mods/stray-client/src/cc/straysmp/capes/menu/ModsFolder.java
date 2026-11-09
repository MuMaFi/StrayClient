package cc.straysmp.capes.menu;

import java.io.IOException;
import java.io.Reader;
import java.nio.file.DirectoryStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;

import cc.straysmp.capes.StrayCapes;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import net.fabricmc.loader.api.FabricLoader;

/**
 * The instance's mods (from the launcher's {@code stray-mods.json} plus any jar dropped into
 * {@code mods/}). A loaded jar can't be swapped while the game runs, so a toggle is stored
 * as the wanted state and applied when the game closes (and by the launcher on the next start).
 */
public final class ModsFolder {
    /** Launcher-managed files that belong to modules, not to the mod list. */
    private static final Set<String> HIDDEN_SLUGS = Set.of("modern-keystrokes");
    private static final Set<String> REQUIRED_SLUGS = Set.of("fabric-api");

    public static final class Entry {
        final String title;
        final String file;
        final boolean locked;
        final boolean enabledNow;
        boolean wanted;

        Entry(String title, String file, boolean locked, boolean enabledNow, boolean wanted) {
            this.title = title;
            this.file = file;
            this.locked = locked;
            this.enabledNow = enabledNow;
            this.wanted = wanted;
        }
    }

    private static boolean hookInstalled;

    private ModsFolder() {}

    private static Path dir() {
        return FabricLoader.getInstance().getGameDir().resolve("mods");
    }

    static List<Entry> list() {
        Path dir = dir();
        var pending = Pending.load();
        List<Entry> out = new ArrayList<>();
        Set<String> known = new HashSet<>();
        Path listFile = FabricLoader.getInstance().getGameDir().resolve("stray-mods.json");
        try (Reader r = Files.newBufferedReader(listFile)) {
            for (JsonElement el : JsonParser.parseReader(r).getAsJsonObject().getAsJsonArray("mods")) {
                JsonObject m = el.getAsJsonObject();
                String file = str(m, "fileName");
                String slug = str(m, "slug");
                if (file.isEmpty()) continue;
                known.add(file);
                boolean on = Files.exists(dir.resolve(file));
                if (!on && !Files.exists(dir.resolve(file + ".disabled"))) continue;
                if (HIDDEN_SLUGS.contains(slug)) continue;
                boolean locked = m.has("locked") && m.get("locked").getAsBoolean() || REQUIRED_SLUGS.contains(slug);
                String title = str(m, "title");
                out.add(new Entry(title.isEmpty() ? file : title, file, locked, on, wanted(pending, file, on)));
            }
        } catch (IOException | RuntimeException ignored) {
            // no launcher list (game started some other way): just show the jars
        }
        try (DirectoryStream<Path> files = Files.newDirectoryStream(dir)) {
            for (Path p : files) {
                String name = p.getFileName().toString();
                boolean on = name.endsWith(".jar");
                String file = on ? name : name.endsWith(".jar.disabled") ? name.substring(0, name.length() - 9) : null;
                if (file == null || known.contains(file)) continue;
                String lower = file.toLowerCase(Locale.ROOT);
                if (lower.startsWith("stray-client") || lower.startsWith("pvp-plus")) continue;
                known.add(file);
                out.add(new Entry(file.substring(0, file.length() - 4), file, false, on, wanted(pending, file, on)));
            }
        } catch (IOException ignored) {
        }
        out.sort(Comparator.comparing((Entry e) -> e.locked).thenComparing(e -> e.title.toLowerCase(Locale.ROOT)));
        return out;
    }

    private static boolean wanted(java.util.Properties pending, String file, boolean now) {
        String v = pending.getProperty("mod." + file);
        return v == null ? now : "on".equals(v);
    }

    private static String str(JsonObject o, String k) {
        return o.has(k) && !o.get(k).isJsonNull() ? o.get(k).getAsString() : "";
    }

    static void toggle(Entry e) {
        if (e.locked) return;
        e.wanted = !e.wanted;
        if (e.wanted == e.enabledNow) Pending.remove("mod." + e.file);
        else Pending.put("mod." + e.file, e.wanted ? "on" : "off");
        installShutdownHook();
    }

    static boolean hasPending() {
        return Pending.load().stringPropertyNames().stream().anyMatch(k -> k.startsWith("mod."));
    }

    /** Rename the jars once the game is closing (works where open files can be renamed). */
    private static synchronized void installShutdownHook() {
        if (hookInstalled) return;
        hookInstalled = true;
        Runtime.getRuntime().addShutdownHook(new Thread(ModsFolder::applyPending, "StrayClient-mods"));
    }

    static void applyPending() {
        Path dir = dir();
        for (String key : Pending.load().stringPropertyNames()) {
            if (!key.startsWith("mod.")) continue;
            String file = key.substring(4);
            if (file.contains("/") || file.contains("\\") || file.contains("..")) continue;
            boolean on = "on".equals(Pending.load().getProperty(key));
            Path jar = dir.resolve(file), off = dir.resolve(file + ".disabled");
            try {
                if (on && Files.exists(off)) Files.move(off, jar);
                else if (!on && Files.exists(jar)) Files.move(jar, off);
            } catch (IOException e) {
                // e.g. Windows keeps the jar locked: the launcher applies it on the next start
                StrayCapes.LOG.info("Mod {} will be switched by the launcher: {}", file, e.toString());
            }
        }
    }
}
