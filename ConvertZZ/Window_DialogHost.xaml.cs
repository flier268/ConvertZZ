using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Linq;
using static ConvertZZ.Enums.Enum_Mode;

namespace ConvertZZ
{
    /// <summary>
    /// Window_DialogHost.xaml 的互動邏輯
    /// </summary>
    public partial class Window_DialogHost : Window
    {
        Pages.Page_AudioTags.Format AudioFormat;
        Mode mode;
        public Window_DialogHost(Mode mode,Pages.Page_AudioTags.Format AudioFormat= Pages.Page_AudioTags.Format.APE)
        {
            this.AudioFormat = AudioFormat;
            this.mode = mode;
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
            switch(((ListBoxItem)((ListBox)sender).SelectedItem).Uid)
            {
                case "Item_File_FileName_Convert":
                    Frame_Report.Content = new Pages.Page_File();
                    break;
                case "Item_Clipboard_Convert":
                    WindowInteropHelper wih = new WindowInteropHelper(this);
                    Frame_Report.Content = new Pages.Page_ClipBoard(wih.Handle);
                    break;
                case "Item_AudioTagConvert":
                    Frame_Report.Content = new Pages.Page_AudioTags(AudioFormat);
                    break;
                case "Item_Exit":
                    Close();
                    break;
            }
        }

        private void Window_Loaded(object sender, RoutedEventArgs e)
        {
            foreach (ListBoxItem item in DemoItemsListBox.Items)
            {
                switch (item.Uid)
                {
                    case "Item_File_FileName_Convert":
                        if (mode == Mode.File_FileName)
                            DemoItemsListBox.SelectedItem = item;
                        break;
                    case "Item_Clipboard_Convert":
                        if (mode == Mode.ClipBoard)
                            DemoItemsListBox.SelectedItem = item;
                        break;
                    case "Item_AudioTagConvert":
                        if (mode == Mode.AutioTag)
                            DemoItemsListBox.SelectedItem = item;
                        break;
                }
            }
        }

        private void DragMove(object sender, MouseButtonEventArgs e)
        {
            if(e.LeftButton== MouseButtonState.Pressed)
            this.DragMove();
        }
    }
}
