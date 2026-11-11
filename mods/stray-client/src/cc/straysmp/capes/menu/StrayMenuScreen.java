package cc.straysmp.capes.menu;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Consumer;
import java.util.function.Supplier;

import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.ChatFormatting;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.Tooltip;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;

/**
 * In-game Stray Client menu (Right Shift): turn modules on/off right away and
 * enable/disable mods for the next start.
 */
public final class StrayMenuScreen extends Screen {
    private static final int BG = 0xF21A1B24;
    private static final int CARD = 0xFF1E1F28;
    private static final int LINE = 0xFF2F3040;
    private static final int ACCENT = 0xFF51A2FF;
    private static final int MUTED = 0xFF9BA0B4;
    private static final int ROW = 22;

    private static int modsPage;

    private final List<ModsFolder.Entry> mods = ModsFolder.list();
    private int px, py, pw, ph, colW, rows;
    private Component status = Component.empty();

    private final Screen parent;

    public StrayMenuScreen() {
        this(null);
    }

    /** @param parent the screen to go back to (the main menu), or null when opened in game */
    public StrayMenuScreen(Screen parent) {
        super(Tr.t("strayclient.menu.title"));
        this.parent = parent;
    }

    @Override
    public void onClose() {
        minecraft.gui.setScreen(parent);
    }

    @Override
    protected void init() {
        pw = Math.min(width - 16, 420);
        ph = Math.min(height - 16, 236);
        px = (width - pw) / 2;
        py = (height - ph) / 2;
        colW = (pw - 30) / 2;
        int top = py + 44;
        rows = Math.max(1, (ph - 44 - 40) / ROW);

        // ── Modules (left) ──
        int lx = px + 10, y = top;
        if (Modules.keystrokesLoaded()) {
            toggle(lx, y, "strayclient.menu.keystrokes", Modules::keystrokes, Modules::setKeystrokes, null);
            y += ROW;
        }
        Button cape = toggle(lx, y, "strayclient.menu.cape", Modules::capeOn, Modules::setCape,
                Tooltip.create(Tr.t("strayclient.menu.cape.tip")));
        cape.active = Modules.hasCapeFile() || Modules.capeOn();
        y += ROW;
        toggle(lx, y, "strayclient.menu.mainmenu", ClientConfig::customMenu, ClientConfig::setCustomMenu,
                Tooltip.create(Tr.t("strayclient.menu.mainmenu.tip")));
        y += ROW;
        if (Modules.pvpLoaded()) {
            toggle(lx, y, "strayclient.menu.pvp.hud", () -> Modules.pvp("hudInfo"), v -> Modules.setPvp("hudInfo", v), null);
            y += ROW;
            toggle(lx, y, "strayclient.menu.pvp.sprint", () -> Modules.pvp("autoSprint"), v -> Modules.setPvp("autoSprint", v), null);
            y += ROW;
            toggle(lx, y, "strayclient.menu.pvp.fullbright", () -> Modules.pvp("fullbright"), v -> Modules.setPvp("fullbright", v), null);
            y += ROW;
            Screen more = Modules.pvpScreen(this);
            if (more != null && y + 20 <= py + ph - 32) {
                addRenderableWidget(Button.builder(Tr.t("strayclient.menu.pvp.more"),
                        b -> minecraft.gui.setScreen(more)).bounds(lx, y, colW, 20).build());
            }
        }

        // ── Mods (right) ──
        int rx = px + 20 + colW;
        int pages = Math.max(1, (mods.size() + rows - 1) / rows);
        modsPage = Math.min(modsPage, pages - 1);
        int from = modsPage * rows;
        for (int i = from; i < Math.min(mods.size(), from + rows); i++) {
            ModsFolder.Entry m = mods.get(i);
            Button b = Button.builder(modLabel(m), btn -> {
                ModsFolder.toggle(m);
                btn.setMessage(modLabel(m));
                status = Tr.t("strayclient.menu.mods.restart").withStyle(ChatFormatting.YELLOW);
            }).bounds(rx, top + (i - from) * ROW, colW, 20).build();
            if (m.locked) {
                b.active = false;
                b.setTooltip(Tooltip.create(Tr.t("strayclient.menu.mods.locked")));
            }
            addRenderableWidget(b);
        }
        if (pages > 1) {
            int by = py + ph - 28;
            addRenderableWidget(Button.builder(Component.literal("<"), b -> { modsPage = (modsPage + pages - 1) % pages; rebuildWidgets(); })
                    .bounds(rx, by, 20, 20).build());
            addRenderableWidget(Button.builder(Component.literal(">"), b -> { modsPage = (modsPage + 1) % pages; rebuildWidgets(); })
                    .bounds(rx + colW - 20, by, 20, 20).build());
        }

        if (Hosting.canHost(minecraft)) {
            int hw = Math.min(120, font.width(Tr.t("strayclient.menu.host")) + 16);
            addRenderableWidget(Button.builder(Tr.t("strayclient.menu.host"), b -> {
                Hosting.publish(minecraft, "");
                onClose();
            }).bounds(px + pw - 10 - hw, py + 5, hw, 18)
                    .tooltip(Tooltip.create(Tr.t("strayclient.menu.host.tip"))).build());
        }
        int half = (colW - 4) / 2;
        addRenderableWidget(Button.builder(Tr.t("strayclient.menu.hud"), b -> minecraft.gui.setScreen(new HudEditScreen(this)))
                .bounds(px + 10, py + ph - 28, half, 20)
                .tooltip(Tooltip.create(Tr.t("strayclient.menu.hud.tip"))).build());
        addRenderableWidget(Button.builder(Tr.t("strayclient.menu.done"), b -> onClose())
                .bounds(px + 10 + half + 4, py + ph - 28, colW - half - 4, 20).build());
        if (ModsFolder.hasPending()) {
            status = Tr.t("strayclient.menu.mods.restart").withStyle(ChatFormatting.YELLOW);
        }
    }

    private Button toggle(int x, int y, String key, Supplier<Boolean> get, Consumer<Boolean> set, Tooltip tip) {
        Button b = Button.builder(onOff(key, get.get()), btn -> {
            set.accept(!get.get());
            btn.setMessage(onOff(key, get.get()));
        }).bounds(x, y, colW, 20).build();
        if (tip != null) b.setTooltip(tip);
        return addRenderableWidget(b);
    }

    private static Component onOff(String key, boolean on) {
        return Tr.t(key).append(": ").append(state(on));
    }

    private static MutableComponent state(boolean on) {
        return Tr.t(on ? "strayclient.menu.on" : "strayclient.menu.off")
                .withStyle(on ? ChatFormatting.GREEN : ChatFormatting.RED);
    }

    private static Component modLabel(ModsFolder.Entry m) {
        String name = m.title.length() > 22 ? m.title.substring(0, 21) + "…" : m.title;
        return Component.literal(name + ": ").append(state(m.wanted));
    }

    @Override
    public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
        super.extractBackground(g, mouseX, mouseY, delta);
        g.fill(px - 1, py - 1, px + pw + 1, py + ph + 1, LINE);
        g.fill(px, py, px + pw, py + ph, BG);
        g.fill(px, py, px + pw, py + 2, ACCENT);
        g.fill(px + 10, py + 30, px + 10 + colW, py + 31, LINE);
        g.fill(px + 20 + colW, py + 30, px + 20 + 2 * colW, py + 31, LINE);
        g.fill(px + 15 + colW, py + 36, px + 16 + colW, py + ph - 34, CARD);
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
        super.extractRenderState(g, mouseX, mouseY, delta);
        g.text(font, Component.literal("STRAY ").append(Component.literal("CLIENT").withColor(ACCENT)), px + 10, py + 8, 0xFFFFFFFF);
        if (!Hosting.canHost(minecraft)) {
            Component hint = Tr.t("strayclient.menu.hint");
            g.text(font, hint, px + pw - 10 - font.width(hint), py + 8, MUTED);
        }
        g.text(font, Tr.t("strayclient.menu.modules"), px + 10, py + 20, MUTED);
        g.text(font, Tr.t("strayclient.menu.mods"), px + 20 + colW, py + 20, MUTED);
        int rx = px + 20 + colW;
        if (mods.isEmpty()) {
            List<net.minecraft.util.FormattedCharSequence> lines = font.split(Tr.t("strayclient.menu.mods.empty"), colW);
            for (int i = 0; i < lines.size(); i++) g.text(font, lines.get(i), rx, py + 48 + i * 10, MUTED);
        } else if (mods.size() > rows) {
            int pages = (mods.size() + rows - 1) / rows;
            String p = (modsPage + 1) + " / " + pages;
            g.text(font, p, rx + (colW - font.width(p)) / 2, py + ph - 22, MUTED);
        }
        if (!Modules.keystrokesLoaded() && !Modules.pvpLoaded()) {
            int y = py + 44 + ROW + 4;
            for (var line : font.split(Tr.t("strayclient.menu.modules.more"), colW)) {
                g.text(font, line, px + 10, y, MUTED);
                y += 10;
            }
        }
        if (!status.getString().isEmpty()) {
            List<net.minecraft.util.FormattedCharSequence> lines = new ArrayList<>(font.split(status, pw - 20));
            g.text(font, lines.get(0), px + 10, py + ph - 40, 0xFFFFFFFF);
        }
    }

    @Override
    public boolean keyPressed(KeyEvent event) {
        if (event.key() == InputConstants.KEY_RSHIFT) {
            onClose();
            return true;
        }
        return super.keyPressed(event);
    }

    @Override
    public boolean isPauseScreen() {
        return false;
    }
}
