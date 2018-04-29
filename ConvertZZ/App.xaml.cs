using System;
using System.IO;
using System.Windows;

namespace ConvertZZ
{
    /// <summary>
    /// App.xaml 的互動邏輯
    /// </summary>
    public partial class App : Application
    {
        public static System.Windows.Forms.NotifyIcon nIcon = new System.Windows.Forms.NotifyIcon();
        private static ChineseConverter _chineseConverter = new ChineseConverter();
        public App()
        {
            //右鍵選單(以後可以加入)：https://blog.csdn.net/DoitPlayer/article/details/72846381
            App.Reload(Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"ConvertZZ.json"));
            foreach (string p in System.IO.Directory.GetFiles(System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Dictionary")))
                ChineseConverter.Load(p);
            ChineseConverter.ReloadFastReplaceDic();


            nIcon.Icon = ConvertZZ.Properties.Resources.favicon;
            nIcon.Visible = true;
            if (App.Settings.ShowBalloonTip)
                nIcon.ShowBalloonTip(1500, "ConvertZZ is here", "ConvertZZ is here", System.Windows.Forms.ToolTipIcon.Info);
            nIcon.Click += nIcon_Click;
        }

        public static ChineseConverter ChineseConverter { get => _chineseConverter; set => _chineseConverter = value; }

        void nIcon_Click(object sender, EventArgs e)
        {
            //events comes here
            MainWindow.Visibility = Visibility.Visible;
            MainWindow.WindowState = WindowState.Normal;
        }
    }
}
