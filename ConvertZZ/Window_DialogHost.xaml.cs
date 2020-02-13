using ConvertZZ.Moudle;
using System;
using System.ComponentModel;
using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using static ConvertZZ.Enums.Enum_Mode;

namespace ConvertZZ
{
    /// <summary>
    /// Window_DialogHost.xaml 的互動邏輯
    /// </summary>
    public partial class Window_DialogHost : Window, INotifyPropertyChanged
    {
        Pages.Page_AudioTags.Format AudioFormat;
        Mode mode;
        string[] FileNames;
        public Window_DialogHost(Mode mode, string[] FileNames, Pages.Page_AudioTags.Format AudioFormat = Pages.Page_AudioTags.Format.APE) : this(mode, AudioFormat)
        {
            this.FileNames = FileNames;
        }
        public Window_DialogHost(Mode mode, Pages.Page_AudioTags.Format AudioFormat = Pages.Page_AudioTags.Format.APE)
        {
            this.AudioFormat = AudioFormat;
            this.mode = mode;
            DataContext = this;
            InitializeComponent();
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

            MenuToggleButton.IsChecked = false;
        }

        private void DemoItemsListBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            switch (((ListBoxItem)((ListBox)sender).SelectedItem).Uid)
            {
                case "Item_File_FileName_Convert":
                    Frame_Report.Content = new Pages.Page_File(FileNames);
                    FileNames = null;
                    CreateShortcutVisibility = File.Exists(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.SendTo), $"{Shortcut_File}.lnk")) ? Visibility.Hidden : Visibility.Visible;
                    break;
                case "Item_Clipboard_Convert":
                    WindowInteropHelper wih = new WindowInteropHelper(this);
                    Frame_Report.Content = new Pages.Page_ClipBoard(wih.Handle);
                    CreateShortcutVisibility = Visibility.Hidden;
                    break;
                case "Item_AudioTagConvert":
                    Frame_Report.Content = new Pages.Page_AudioTags(AudioFormat, FileNames);
                    FileNames = null;
                    CreateShortcutVisibility = File.Exists(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.SendTo), $"{Shortcut_Audio}.lnk")) ? Visibility.Hidden : Visibility.Visible;
                    break;
                case "Item_Exit":
                    Close();
                    break;
            }
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            DemoItemsListBox.SelectedItem = DemoItemsListBox.Items.GetItemAt((int)mode);
        }

        private void DragMove(object sender, MouseButtonEventArgs e)
        {
            if (e.LeftButton == MouseButtonState.Pressed)
                this.DragMove();
        }
        const string Shortcut_File = "ConvertZZ(文件轉換)";
        const string Shortcut_Audio = "ConvertZZ(Audio標籤轉換)";
        private void Button_CreateShortCut(object sender, RoutedEventArgs e)
        {
            string ShortchuName = "";
            string arg = "";
            if (Frame_Report.Content.GetType() == typeof(Pages.Page_File))
            {
                ShortchuName = Shortcut_File;
                arg = "/file";
            }
            else if (Frame_Report.Content.GetType() == typeof(Pages.Page_AudioTags))
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
                CreateShortcutVisibility = Visibility.Hidden;
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

        public Visibility CreateShortcutVisibility { get; set; }

        public event PropertyChangedEventHandler PropertyChanged;


    }
}
