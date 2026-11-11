package cc.straysmp.capes.menu;

import net.minecraft.SharedConstants;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.Tooltip;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.client.gui.screens.multiplayer.SafetyScreen;
import net.minecraft.client.gui.screens.options.OptionsScreen;
import net.minecraft.client.gui.screens.worldselection.SelectWorldScreen;
import net.minecraft.network.chat.Component;

/**
 * The Stray Client main menu: a panel on the left with the logo and the main buttons over
 * Minecraft's panorama. Replaces the vanilla title screen while the option is on
 * (Right Shift menu, or the "Classic menu" button here).
 */
public final class StrayTitleScreen extends Screen {
    private static final int PANEL = 0xE61A1B24;
    private static final int LINE = 0xFF2F3040;
    private static final int ACCENT = 0xFF51A2FF;
    private static final int MUTED = 0xFF9BA0B4;

    private int panelW;

    public StrayTitleScreen() {
        super(Component.literal("Stray Client"));
    }

    /** Swap the vanilla title screen for ours (called every client tick). */
    public static void tick(Minecraft mc) {
        Screen s = mc.gui.screen();
        if (s != null && s.getClass() == TitleScreen.class && mc.gui.overlay() == null && ClientConfig.customMenu()) {
            mc.gui.setScreen(new StrayTitleScreen());
        }
    }

    @Override
    protected void init() {
        panelW = Math.max(170, Math.min(230, width / 3));
        int bw = panelW - 40, x = 20, gap = 24;
        int y = Math.max(74, height / 2 - 62);
        addRenderableWidget(Button.builder(Tr.t("strayclient.title.singleplayer"), b -> minecraft.gui.setScreen(new SelectWorldScreen(this)))
                .bounds(x, y, bw, 20).build());
        addRenderableWidget(Button.builder(Tr.t("strayclient.title.multiplayer"), b -> {
            Screen next = minecraft.options.skipMultiplayerWarning ? new JoinMultiplayerScreen(this) : new SafetyScreen(this);
            minecraft.gui.setScreen(next);
        }).bounds(x, y + gap, bw, 20).build());
        addRenderableWidget(Button.builder(Tr.t("strayclient.title.mods"), b -> minecraft.gui.setScreen(new StrayMenuScreen(this)))
                .bounds(x, y + 2 * gap, bw, 20)
                .tooltip(Tooltip.create(Tr.t("strayclient.title.mods.tip"))).build());
        addRenderableWidget(Button.builder(Tr.t("strayclient.title.options"), b -> minecraft.gui.setScreen(new OptionsScreen(this, minecraft.options)))
                .bounds(x, y + 3 * gap, bw, 20).build());
        addRenderableWidget(Button.builder(Tr.t("strayclient.title.quit"), b -> minecraft.stop())
                .bounds(x, y + 4 * gap + 8, bw, 20).build());
        addRenderableWidget(Button.builder(Tr.t("strayclient.title.classic"), b -> {
            ClientConfig.setCustomMenu(false);
            minecraft.gui.setScreen(new TitleScreen());
        }).bounds(x, height - 44, bw, 16)
                .tooltip(Tooltip.create(Tr.t("strayclient.title.classic.tip"))).build());
    }

    @Override
    public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
        extractPanorama(g, delta);
        g.fill(0, 0, panelW, height, PANEL);
        g.fill(panelW, 0, panelW + 1, height, LINE);
        g.fill(0, 0, panelW, 2, ACCENT);
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
        super.extractRenderState(g, mouseX, mouseY, delta);
        // logo: "STRAY" white over "CLIENT" blue, drawn large with the pixel font
        float scale = panelW >= 200 ? 3f : 2.5f;
        g.pose().pushMatrix();
        g.pose().translate(20, 22);
        g.pose().scale(scale, scale);
        g.text(font, "STRAY", 0, 0, 0xFFFFFFFF);
        g.text(font, "CLIENT", 0, 10, ACCENT);
        g.pose().popMatrix();
        String version = "Minecraft " + SharedConstants.getCurrentVersion().name();
        g.text(font, version, 20, height - 22, MUTED);
        g.text(font, Tr.s("strayclient.title.hint"), 20, height - 12, MUTED);
    }

    @Override
    public boolean shouldCloseOnEsc() {
        return false;
    }

    @Override
    public boolean isPauseScreen() {
        return false;
    }
}
