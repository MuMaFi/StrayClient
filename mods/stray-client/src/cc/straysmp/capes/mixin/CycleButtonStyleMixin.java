package cc.straysmp.capes.mixin;

import cc.straysmp.capes.menu.StrayStyle;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.CycleButton;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The option buttons ("Difficulty: Normal" …) in the Stray look. */
@Mixin(CycleButton.class)
public abstract class CycleButtonStyleMixin {
    @Inject(method = "extractContents", at = @At("HEAD"), cancellable = true, require = 0)
    private void strayclient$style(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta, CallbackInfo ci) {
        if (!StrayStyle.on()) return;
        StrayStyle.button(g, (AbstractWidget) (Object) this);
        ci.cancel();
    }
}
