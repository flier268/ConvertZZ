using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using ConvertZZ.ViewModels;

namespace ConvertZZ.Views
{
    /// <summary>
    /// Window_Setting.xaml 的互動邏輯
    /// </summary>
    public partial class Window_Setting : Window
    {
        public Window_Setting()
        {
            InitializeComponent();
            ViewModel = (SettingViewModel)DataContext;
        }

        internal SettingViewModel ViewModel { get; }

        private void TextBox_ShortCut_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            e.Handled = true;
            if (sender is TextBox textBox)
            {
                textBox.Text = (e.Key == Key.System ? e.SystemKey : e.Key == Key.ImeProcessed ? e.ImeProcessedKey : e.Key).ToString();
            }
        }

        private void TextBox_ShortCut_Modify_PreviewKeyDown(object sender, KeyEventArgs e)
        {
            e.Handled = true;
            if (sender is TextBox textBox)
            {
                textBox.Text = e.KeyboardDevice.Modifiers.ToString();
            }
        }

        private void Button_FilterEditor_Click(object sender, RoutedEventArgs e)
        {
            App.Settings.FileConvert.CallFilterEditor();
            ViewModel.TypeFilter = App.Settings.FileConvert.TypeFilter;
        }

        private async void Button_DictionaryEdit_Click(object sender, RoutedEventArgs e)
        {
            //new Window_DictionaryEditor().ShowDialog();
        }

        private void Button_FanhuajiSetting_Click(object sender, RoutedEventArgs e)
        {
            new Window_FanhuajiSetting().ShowDialog();
        }

        private void RadioButton_Fanhuaji_Checked(object sender, RoutedEventArgs e)
        {
            if (!this.IsLoaded)
                return;
            if (Moudle.Window_MessageBoxEx.ShowDialog(Fanhuaji_API.Fanhuaji.Terms_of_Service, "使用繁化姬API須接受以下條約", "我不同意", "我同意") != Moudle.Window_MessageBoxEx.MessageBoxExResult.B)
                ViewModel.UseLocalDic = true;
        }

        private async void Window_Closing(object sender, CancelEventArgs e)
        {
        }
    }
}