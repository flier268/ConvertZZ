using System;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using ConvertZZ.Core.Helpers;
using ConvertZZ.Moudle;
using ConvertZZ.ViewModels;

namespace ConvertZZ.Views
{
    /// <summary>
    /// Window_DialogHost.xaml 的互動邏輯
    /// </summary>
    public partial class Window_DialogHost : Window
    {
        private EAudioFormat AudioFormat;
        private EMode mode;
        private string[]? FileNames;
        private DialogHostViewModel ViewModel { get; }

        public Window_DialogHost(EMode mode, EAudioFormat AudioFormat = EAudioFormat.APE)
        {
            this.AudioFormat = AudioFormat;
            this.mode = mode;
            InitializeComponent();
            ViewModel = (DialogHostViewModel)DataContext;
        }

        public Window_DialogHost(EMode mode, string[] FileNames, EAudioFormat AudioFormat = EAudioFormat.APE) : this(mode, AudioFormat)
        {
            this.FileNames = FileNames;
        }

        private void UIElement_OnPreviewMouseLeftButtonUp(object sender, MouseButtonEventArgs e)
        {
            //until we had a StaysOpen glag to Drawer, this will help with scroll bars
            var dependencyObject = Mouse.Captured as DependencyObject;
            while (dependencyObject != null)
            {
                if (dependencyObject is ScrollBar) return;
                dependencyObject = VisualTreeHelper.GetParent(dependencyObject);
            }

            ViewModel.MenuToggleButtonIsChecked = false;
        }

        private void DemoItemsListBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            switch (((ListBoxItem)((ListBox)sender).SelectedItem).Uid)
            {
                case "Item_File_FileName_Convert":
                    ViewModel.Frame_Report = new Pages.Page_File(FileNames);
                    FileNames = null;
                    ViewModel.CreateShortcutVisibility = File.Exists(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.SendTo), $"{Shortcut_File}.lnk")) ? Visibility.Hidden : Visibility.Visible;
                    break;

                case "Item_Clipboard_Convert":
                    WindowInteropHelper wih = new(this);
                    ViewModel.Frame_Report = new Pages.Page_ClipBoard(wih.Handle);
                    ViewModel.CreateShortcutVisibility = Visibility.Hidden;
                    break;

                case "Item_AudioTagConvert":
                    ViewModel.Frame_Report = new Pages.Page_AudioTags(AudioFormat, FileNames);
                    FileNames = null;
                    ViewModel.CreateShortcutVisibility = File.Exists(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.SendTo), $"{Shortcut_Audio}.lnk")) ? Visibility.Hidden : Visibility.Visible;
                    break;

                case "Item_Exit":
                    Close();
                    break;
            }
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            ViewModel.ListBoxItemSelectedIndex = (int)mode;
        }

        private void DragMove(object sender, MouseButtonEventArgs e)
        {
            if (e.LeftButton == MouseButtonState.Pressed)
                this.DragMove();
        }

        private const string Shortcut_File = "ConvertZZ(文件轉換)";
        private const string Shortcut_Audio = "ConvertZZ(Audio標籤轉換)";

        private void Button_CreateShortCut(object sender, RoutedEventArgs e)
        {
            string ShortchuName = "";
            string arg = "";
            if (ViewModel.Frame_Report?.GetType() == typeof(Pages.Page_File))
            {
                ShortchuName = Shortcut_File;
                arg = "/file";
            }
            else if (ViewModel.Frame_Report?.GetType() == typeof(Pages.Page_AudioTags))
            {
                ShortchuName = Shortcut_Audio;
                arg = "/audio";
            }
            if (Moudle.Window_MessageBoxEx.ShowDialog($"添加\"{ShortchuName}\"捷徑至傳送到", "建立捷徑", "是", "否") == Moudle.Window_MessageBoxEx.MessageBoxExResult.A)
            {
                string ShortcutPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.SendTo), $"{ShortchuName}.lnk");
                if (File.Exists(ShortcutPath))
                    File.Delete(ShortcutPath);
                Moudle.Shortcut.Create(ShortcutPath, System.Diagnostics.Process.GetCurrentProcess().MainModule.FileName, arg, AppDomain.CurrentDomain.BaseDirectory, "簡繁轉換工具", "", "");
                new Toast("捷徑已建立").Show();
                ViewModel.CreateShortcutVisibility = Visibility.Hidden;
            }
        }

        private void Button_Minimize(object sender, RoutedEventArgs e)
        {
            WindowState = WindowState.Minimized;
        }

        private void Button_Close(object sender, RoutedEventArgs e)
        {
            this.Close();
        }
    }
}