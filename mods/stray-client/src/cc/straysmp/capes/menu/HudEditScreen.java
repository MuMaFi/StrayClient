package cc.straysmp.capes.menu;

import java.util.ArrayList;
import java.util.List;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

/**
 * "Move HUD": every active module gets a frame that can be dragged anywhere on screen with
 * the mouse. The real HUD moves along live; positions are saved when the screen closes.
 */
public final class HudEditScreen extends Screen {
    private static final int FRAME = 0xFF51A2FF;
    private static final int FILL = 0x5551A2FF;
    private static final int FILL_HOT = 0x8851A2FF;

    /** A draggable module frame. */
    private abstract static class Box {
        final Component label;
        final int w, h;

        Box(Component label, int w, int h) {
            this.label = label;
            this.w = w;
            this.h = h;
        }

        abstract int x();
        abstract int y();
        abstract void moveTo(int x, int y);
        abstract void reset();
    }

    private final Screen parent;
    private final List<Box> boxes = new ArrayList<>();
    private Box dragging;
    private double grabX, grabY;

    public HudEditScreen(Screen parent) {
        super(Component.translatable("strayclient.hud.title"));
        this.parent = parent;
    }

    @Override
    protected void init() {
        HudOffsets.load();
        boxes.clear();
        if (Modules.keystrokesLoaded() && Modules.keystrokes()) {
            float scale = Math.max(0.5f, Modules.keystrokesScale());
            boxes.add(new Box(Component.literal("Keystrokes"), Math.round(66 * scale), Math.round(100 * scale)) {
                @Override int x() { return Modules.keystrokesX(); }
                @Override int y() { return Modules.keystrokesY(); }
                @Override void moveTo(int x, int y) { Modules.setKeystrokesPos(x, y); }
                @Override void reset() { Modules.setKeystrokesPos(25, Modules.pvpLoaded() ? 80 : 10); }
            });
        }
        if (Modules.pvpLoaded()) {
            if (Modules.pvp("hudInfo") || Modules.pvp("hudEffects")) {
                boxes.add(new Box(Component.translatable("strayclient.hud.info"), 150, 72) {
                    @Override int x() { return 2 + HudOffsets.infoDx; }
                    @Override int y() { return 2 + HudOffsets.infoDy; }
                    @Override void moveTo(int x, int y) { HudOffsets.infoDx = x - 2; HudOffsets.infoDy = y - 2; }
                    @Override void reset() { HudOffsets.infoDx = 0; HudOffsets.infoDy = 0; }
                });
            }
            if (Modules.pvp("hudArmor") || Modules.pvp("hudItems")) {
                final int baseY = height / 2 - 72;
                boxes.add(new Box(Component.translatable("strayclient.hud.armor"), 76, 112) {
                    @Override int x() { return 2 + HudOffsets.armorDx; }
                    @Override int y() { return baseY + HudOffsets.armorDy; }
                    @Override void moveTo(int x, int y) { HudOffsets.armorDx = x - 2; HudOffsets.armorDy = y - baseY; }
                    @Override void reset() { HudOffsets.armorDx = 0; HudOffsets.armorDy = 0; }
                });
            }
        }
        int bw = 110, by = height - 28;
        addRenderableWidget(Button.builder(Component.translatable("strayclient.hud.reset"), b -> boxes.forEach(Box::reset))
                .bounds(width / 2 - bw - 3, by, bw, 20).build());
        addRenderableWidget(Button.builder(Component.translatable("strayclient.menu.done"), b -> onClose())
                .bounds(width / 2 + 3, by, bw, 20).build());
    }

    private Box boxAt(double mx, double my) {
        for (int i = boxes.size() - 1; i >= 0; i--) {
            Box b = boxes.get(i);
            if (mx >= b.x() && mx < b.x() + b.w && my >= b.y() && my < b.y() + b.h) return b;
        }
        return null;
    }

    @Override
    public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
        if (super.mouseClicked(event, doubleClick)) return true;
        Box b = event.button() == 0 ? boxAt(event.x(), event.y()) : null;
        if (b == null) return false;
        dragging = b;
        grabX = event.x() - b.x();
        grabY = event.y() - b.y();
        return true;
    }

    @Override
    public boolean mouseDragged(MouseButtonEvent event, double dx, double dy) {
        if (dragging == null) return super.mouseDragged(event, dx, dy);
        int x = (int) Math.round(event.x() - grabX), y = (int) Math.round(event.y() - grabY);
        // keep the frame on screen
        x = Math.max(0, Math.min(width - dragging.w, x));
        y = Math.max(0, Math.min(height - dragging.h, y));
        dragging.moveTo(x, y);
        return true;
    }

    @Override
    public boolean mouseReleased(MouseButtonEvent event) {
        if (dragging != null) {
            dragging = null;
            return true;
        }
        return super.mouseReleased(event);
    }

    @Override
    public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
        // no blur: the game and the real HUD stay visible while moving things around
        g.fill(0, 0, width, height, 0x33000000);
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
        for (Box b : boxes) {
            boolean hot = b == dragging || (dragging == null && b == boxAt(mouseX, mouseY));
            g.fill(b.x(), b.y(), b.x() + b.w, b.y() + b.h, hot ? FILL_HOT : FILL);
            g.outline(b.x(), b.y(), b.w, b.h, FRAME);
            g.text(font, b.label, b.x() + 4, b.y() + b.h - 11, 0xFFFFFFFF);
        }
        super.extractRenderState(g, mouseX, mouseY, delta);
        Component hint = boxes.isEmpty() ? Component.translatable("strayclient.hud.empty") : title;
        g.centeredText(font, hint, width / 2, height - 42, 0xFFFFFFFF);
    }

    @Override
    public void removed() {
        HudOffsets.save();
        Modules.saveKeystrokes();
    }

    @Override
    public void onClose() {
        minecraft.gui.setScreen(parent);
    }

    @Override
    public boolean isPauseScreen() {
        return false;
    }
}
