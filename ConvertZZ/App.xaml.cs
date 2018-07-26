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

        public App()
        {
            App.Reload(Path.Combine(AppDomain.CurrentDomain.BaseDirectory,"ConvertZZ.json"));
            foreach (string p in System.IO.Directory.GetFiles(System.IO.Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Dictionary")))
                ChineseConverter.Load(p);
            ChineseConverter.ReloadFastReplaceDic();
            
            nIcon.Icon = ConvertZZ.Properties.Resources.icon;
            nIcon.Visible = true;
        }

        public static ChineseConverter ChineseConverter { get; set; } = new ChineseConverter();
    }
}
