using System;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Animation;

namespace ColorfulControlCenter.Installer
{
    internal sealed partial class InstallerWindow
    {
        private readonly TranslateTransform bodyOffset = new TranslateTransform();
        private FrameworkElement actions;
        private int transitionSerial;

        private bool MotionAllowed
        {
            get { return SystemParameters.ClientAreaAnimation && !SystemParameters.HighContrast
                && Measure("MotionMilliseconds") > 0; }
        }
        private void AnimateStep(InstallerStep previousStep)
        {
            if (previousStep == step || !IsLoaded || !MotionAllowed)
            {
                transitionSerial++;
                body.BeginAnimation(OpacityProperty, null);
                bodyOffset.BeginAnimation(TranslateTransform.XProperty, null);
                actions.BeginAnimation(OpacityProperty, null);
                return;
            }
            int serial = ++transitionSerial;
            double distance = Measure("MotionDistance") * (step > previousStep ? 1 : -1);
            var fade = Tween(0, 1, "MotionMilliseconds");
            var offset = Tween(distance, 0, "MotionMilliseconds");
            var actionFade = Tween(0.65, 1, "FastMotionMilliseconds");
            fade.Completed += delegate { if (serial == transitionSerial) body.BeginAnimation(OpacityProperty, null); };
            offset.Completed += delegate { if (serial == transitionSerial) bodyOffset.BeginAnimation(TranslateTransform.XProperty, null); };
            actionFade.Completed += delegate { if (serial == transitionSerial) actions.BeginAnimation(OpacityProperty, null); };
            body.BeginAnimation(OpacityProperty, fade, HandoffBehavior.SnapshotAndReplace);
            bodyOffset.BeginAnimation(TranslateTransform.XProperty, offset, HandoffBehavior.SnapshotAndReplace);
            actions.BeginAnimation(OpacityProperty, actionFade, HandoffBehavior.SnapshotAndReplace);
        }

        private DoubleAnimation Tween(double from, double to, string duration)
        {
            return new DoubleAnimation(from, to, TimeSpan.FromMilliseconds(Measure(duration))) {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }, FillBehavior = FillBehavior.Stop,
            };
        }

    }
}
